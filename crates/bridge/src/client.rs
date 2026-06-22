use std::sync::Arc;

use neon::prelude::*;

use crate::error::create_js_error;
use crate::RUNTIME;
use kurrentdb::{
    Authentication, Client, ClientSettings, Position, ReadAllOptions, ReadStream,
    ReadStreamOptions, RecordedEvent, ResolvedEvent, StreamPosition,
};
use neon::{
    prelude::FunctionContext,
    result::JsResult,
    types::{JsBigInt, JsBoolean, JsFunction, JsObject, JsPromise, JsString, JsValue},
};
use tokio::sync::{Mutex};

pub fn create(mut cx: FunctionContext) -> JsResult<JsObject> {
    let conn_string = cx.argument::<JsString>(0)?.value(&mut cx);

    let setts = match conn_string.parse::<ClientSettings>() {
        Err(e) => {
            let js_error = create_js_error(&mut cx, e)?;
            return cx.throw(js_error)
        }
        Ok(s) => s,
    };

    let client = match Client::with_runtime_handle(RUNTIME.handle().clone(), setts) {
        Err(e) => {
            let js_error = create_js_error(&mut cx, e)?;
            cx.throw(js_error)?
        }
        Ok(c) => c,
    };

    let obj = cx.empty_object();

    let local_client = client.clone();
    let client_read_stream =
        JsFunction::new(&mut cx, move |cx| read_stream(local_client.clone(), cx))?;

    let local_client = client.clone();
    let client_read_all = JsFunction::new(&mut cx, move |cx| read_all(local_client.clone(), cx))?;

    obj.set(&mut cx, "readStream", client_read_stream)?;
    obj.set(&mut cx, "readAll", client_read_all)?;

    Ok(obj)
}

pub enum Options {
    Regular {
        stream_name: String,
        options: ReadStreamOptions,
    },

    All(ReadAllOptions),
}

fn authentication_from_js<'a>(
    cx: &mut FunctionContext<'a>,
    obj: Handle<'a, JsObject>,
) -> NeonResult<Authentication> {
    let bearer_value = obj.get_value(cx, "bearerToken")?;
    if !bearer_value.is_a::<JsUndefined, _>(cx) {
        let token = match bearer_value.downcast::<JsString, _>(cx) {
            Ok(s) => s.value(cx),
            Err(_) => {
                return cx.throw_type_error("credentials.bearerToken must be a string");
            }
        };
        return Ok(Authentication::bearer(token));
    }

    let username = obj
        .get_value(cx, "username")?
        .downcast::<JsString, _>(cx)
        .ok();
    let password = obj
        .get_value(cx, "password")?
        .downcast::<JsString, _>(cx)
        .ok();

    match (username, password) {
        (Some(u), Some(p)) => {
            let login = u.value(cx);
            let password = p.value(cx);
            Ok(Authentication::basic(login, password))
        }
        _ => cx.throw_type_error(
            "credentials must include either { bearerToken } or { username, password }",
        ),
    }
}

pub fn read_stream(client: Client, mut cx: FunctionContext) -> JsResult<JsPromise> {
    let stream_name = cx.argument::<JsString>(0)?.value(&mut cx);
    let params = if cx.len() >= 2 {
        if let Ok(arg) = cx.argument::<JsValue>(1) {
            arg.downcast::<JsObject, _>(&mut cx)
                .unwrap_or_else(|_| cx.empty_object())
        } else {
            cx.empty_object()
        }
    } else {
        cx.empty_object()
    };
    let mut options = ReadStreamOptions::default();

    let direction_str = match params.get_opt::<JsString, _, _>(&mut cx, "direction")? {
        Some(s) => s.value(&mut cx),
        None => "forwards".to_string(),
    };
    options = match direction_str.as_str() {
        "forwards" => options.forwards(),
        "backwards" => options.backwards(),
        x => return cx.throw_error(format!("invalid direction value: '{}'", x)),
    };

    if let Some(value) = params.get_opt::<JsValue, _, _>(&mut cx, "fromRevision")? {
        if let Ok(s) = value.downcast::<JsString, _>(&mut cx) {
            options = match s.value(&mut cx).as_str() {
                "start" => options.position(StreamPosition::Start),
                "end" => options.position(StreamPosition::End),
                x => return cx.throw_error(format!("invalid fromRevision value: '{}'", x)),
            };
        } else if let Ok(n) = value.downcast::<JsBigInt, _>(&mut cx) {
            match n.to_u64(&mut cx) {
                Ok(r) => options = options.position(StreamPosition::Position(r)),
                Err(e) => return cx.throw_error(e.to_string()),
            };
        } else {
            return cx.throw_error("fromRevision can only be 'start', 'end' or a bigint");
        }
    }

    if let Some(obj) = params.get_opt::<JsObject, _, _>(&mut cx, "credentials")? {
        options = options.authenticated(authentication_from_js(&mut cx, obj)?);
    }

    if let Some(js_bigint) = params.get_opt::<JsBigInt, _, _>(&mut cx, "maxCount")? {
        match js_bigint.to_u64(&mut cx) {
            Ok(r) => options = options.max_count(r as usize),
            Err(e) => return cx.throw_error(e.to_string()),
        }
    }

    let require_leader = params
        .get_opt::<JsBoolean, _, _>(&mut cx, "requiresLeader")?
        .map(|b| b.value(&mut cx))
        .unwrap_or(false);
    options = options.requires_leader(require_leader);

    let resolve_links = params
        .get_opt::<JsBoolean, _, _>(&mut cx, "resolvesLink")?
        .map(|b| b.value(&mut cx))
        .unwrap_or(false);
    options = if resolve_links {
        options.resolve_link_tos()
    } else {
        options
    };

    let options = Options::Regular {
        stream_name,
        options,
    };

    read_internal(client, options, cx)
}

pub fn read_all(client: Client, mut cx: FunctionContext) -> JsResult<JsPromise> {
    let params = if !cx.is_empty() {
        if let Ok(arg) = cx.argument::<JsValue>(0) {
            arg.downcast::<JsObject, _>(&mut cx)
                .unwrap_or_else(|_| cx.empty_object())
        } else {
            cx.empty_object()
        }
    } else {
        cx.empty_object()
    };

    let mut options = ReadAllOptions::default();

    let direction_str = match params.get_opt::<JsString, _, _>(&mut cx, "direction")? {
        Some(s) => s.value(&mut cx),
        None => "forwards".to_string(),
    };
    options = match direction_str.as_str() {
        "forwards" => options.forwards(),
        "backwards" => options.backwards(),
        x => cx.throw_error(format!("invalid direction value: '{}'", x))?,
    };

    options = if let Some(value) = params.get_opt::<JsValue, _, _>(&mut cx, "fromPosition")? {
        if let Ok(s) = value.downcast::<JsString, _>(&mut cx) {
            match s.value(&mut cx).as_str() {
                "start" => options.position(StreamPosition::Start),
                "end" => options.position(StreamPosition::End),
                x => cx.throw_error(format!("invalid fromPosition value: '{}'", x))?,
            }
        } else if let Ok(obj) = value.downcast::<JsObject, _>(&mut cx) {
            let commit = obj
                .get::<JsBigInt, _, _>(&mut cx, "commit")?
                .to_u64(&mut cx);
            let prepare = obj
                .get::<JsBigInt, _, _>(&mut cx, "prepare")?
                .to_u64(&mut cx);

            let position = commit.and_then(|commit| {
                prepare.map(|prepare| StreamPosition::Position(Position { commit, prepare }))
            });

            match position {
                Ok(p) => options.position(p),
                Err(e) => cx.throw_error(e.to_string())?,
            }
        } else {
            cx.throw_error(
                "fromPosition can only be 'start', 'end' or an object with 'commit' and 'prepare'",
            )?
        }
    } else {
        options.position(StreamPosition::Start)
    };

    options = if let Some(obj) = params.get_opt::<JsObject, _, _>(&mut cx, "credentials")? {
        options.authenticated(authentication_from_js(&mut cx, obj)?)
    } else {
        options
    };

    options = if let Some(js_bigint) = params.get_opt::<JsBigInt, _, _>(&mut cx, "maxCount")? {
        match js_bigint.to_u64(&mut cx) {
            Ok(r) => options.max_count(r as usize),
            Err(e) => return cx.throw_error(e.to_string()),
        }
    } else {
        options
    };

    let require_leader = params
        .get_opt::<JsBoolean, _, _>(&mut cx, "requiresLeader")?
        .map(|b| b.value(&mut cx))
        .unwrap_or(false);
    options = options.requires_leader(require_leader);

    let resolve_links = params
        .get_opt::<JsBoolean, _, _>(&mut cx, "resolvesLink")?
        .map(|b| b.value(&mut cx))
        .unwrap_or(false);
    options = if resolve_links {
        options.resolve_link_tos()
    } else {
        options
    };

    if let Some(filter_obj) = params.get_opt::<JsObject, _, _>(&mut cx, "filter")? {
        let filter_on = filter_obj
            .get_opt::<JsString, _, _>(&mut cx, "filterOn")?
            .map(|s| s.value(&mut cx))
            .unwrap_or_else(|| "".to_string());

        let mut subscription_filter = match filter_on.as_str() {
            "streamName" => kurrentdb::SubscriptionFilter::on_stream_name(),
            "eventType" => kurrentdb::SubscriptionFilter::on_event_type(),
            _ => cx.throw_error("filter.filterOn must be 'streamName' or 'eventType'")?,
        };

        if let Some(prefixes_array) = filter_obj.get_opt::<JsArray, _, _>(&mut cx, "prefixes")? {
            let prefixes_len = prefixes_array.len(&mut cx);
            for i in 0..prefixes_len {
                let prefix = prefixes_array
                    .get::<JsString, _, _>(&mut cx, i)?
                    .value(&mut cx);
                subscription_filter = subscription_filter.add_prefix(&prefix);
            }
        }

        if let Some(regex) = filter_obj.get_opt::<JsString, _, _>(&mut cx, "regex")? {
            subscription_filter = subscription_filter.regex(regex.value(&mut cx));
        }

        options = options.filter(subscription_filter);
    }

    read_internal(client, Options::All(options), cx)
}

fn read_internal(client: Client, options: Options, mut cx: FunctionContext) -> JsResult<JsPromise> {
    let channel = cx.channel();
    let (deferred, promise) = cx.promise();
    RUNTIME.spawn(async move {
        let result = match options {
            Options::Regular {
                stream_name,
                options,
            } => client.read_stream(stream_name.as_str(), &options).await,

            Options::All(options) => client.read_all(&options).await,
        };

        deferred.settle_with(&channel, |mut cx| match result {
            Err(e) => {
                let js_error = create_js_error(&mut cx, e)?;
                cx.throw(js_error)
            }
            Ok(stream) => read_stream_ref(&mut cx, stream),
        });
    });

    Ok(promise)
}

fn js_recorded_event<'a, C: Context<'a>>(
    cx: &mut C,
    event: &RecordedEvent,
) -> JsResult<'a, JsObject> {
    let obj = cx.empty_object();

    let stream_id = cx.string(event.stream_id());
    obj.set(cx, "streamId", stream_id)?;

    let id = cx.string(event.id.to_string());
    obj.set(cx, "id", id)?;

    let event_type = cx.string(event.event_type.as_str());
    obj.set(cx, "type", event_type)?;

    let is_json = cx.boolean(event.is_json);
    obj.set(cx, "isJson", is_json)?;

    let revision = JsBigInt::from_u64(cx, event.revision);
    obj.set(cx, "revision", revision)?;

    let created = cx.number(event.created.timestamp_millis() as f64);
    obj.set(cx, "created", created)?;

    let data = JsBuffer::from_slice(cx, &event.data)?;
    obj.set(cx, "data", data)?;

    let metadata = JsBuffer::from_slice(cx, &event.custom_metadata)?;
    obj.set(cx, "metadata", metadata)?;

    let position = cx.empty_object();
    let commit = JsBigInt::from_u64(cx, event.position.commit);
    position.set(cx, "commit", commit)?;
    let prepare = JsBigInt::from_u64(cx, event.position.prepare);
    position.set(cx, "prepare", prepare)?;
    obj.set(cx, "position", position)?;

    Ok(obj)
}

fn js_resolve_event<'a, C: Context<'a>>(
    cx: &mut C,
    event: &ResolvedEvent,
) -> JsResult<'a, JsObject> {
    let obj = cx.empty_object();

    if let Some(event) = event.event.as_ref() {
        let recorded = js_recorded_event(cx, event)?;
        obj.set(cx, "event", recorded)?;
    }

    if let Some(link) = event.link.as_ref() {
        let recorded = js_recorded_event(cx, link)?;
        obj.set(cx, "link", recorded)?;
    }

    if let Some(commit_position) = event.commit_position {
        let commit_position = JsBigInt::from_u64(cx, commit_position);
        obj.set(cx, "commitPosition", commit_position)?;
    }

    Ok(obj)
}

pub fn read_stream_next_mutex(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let sender = cx.argument::<JsBox<ReadStreamRef>>(0)?.inner.clone();
    let (deferred, promise) = cx.promise();
    let channel = cx.channel();

    RUNTIME.spawn(async move {
        let result = {
            let mut stream = sender.lock().await;
            let batch_size = 64usize;
            let mut batch = Vec::with_capacity(batch_size);

            loop {
                match stream.next().await {
                    Err(e) => break Err(e),
                    Ok(event) => {
                        if let Some(event) = event {
                            batch.push(event);
                        } else if batch.is_empty() {
                            break Ok(None);
                        } else {
                            break Ok(Some(batch));
                        }

                        if batch.len() >= batch_size {
                            break Ok(Some(batch));
                        }
                    }
                }
            }
        };

        deferred.settle_with(&channel, |mut cx| match result {
            Err(e) => {
                let js_error = create_js_error(&mut cx, e)?;
                cx.throw(js_error)
            }
            Ok(events) => {
                let result = cx.empty_object();

                match events {
                    Some(events) => {
                        let array = JsArray::new(&mut cx, events.len());
                        for (index, event) in events.iter().enumerate() {
                            let resolved = js_resolve_event(&mut cx, event)?;
                            array.set(&mut cx, index as u32, resolved)?;
                        }
                        result.set(&mut cx, "value", array)?;
                        let done = cx.boolean(false);
                        result.set(&mut cx, "done", done)?;
                    }

                    None => {
                        let array = JsArray::new(&mut cx, 0);
                        result.set(&mut cx, "value", array)?;
                        let done = cx.boolean(true);
                        result.set(&mut cx, "done", done)?;
                    }
                }

                Ok(result)
            }
        });
    });

    Ok(promise)
}

struct ReadStreamRef {
    inner: Arc<Mutex<ReadStream>>,
}

impl ReadStreamRef {
    fn new(inner: ReadStream) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }
}

impl Finalize for ReadStreamRef {}

fn read_stream_ref<'a, C>(cx: &mut C, stream: ReadStream) -> JsResult<'a, JsBox<ReadStreamRef>>
where
    C: Context<'a>,
{
    Ok(JsBox::new(cx, ReadStreamRef::new(stream)))
}

// Copyright (C) 2016-2018 Sebastian Dröge <sebastian@centricular.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future;
use futures::prelude::*;
use reqwest::{Client, Response, StatusCode};
use tokio::runtime;
use url::Url;

use std::sync::LazyLock;

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::base_src::CreateSuccess;
use gst_base::subclass::prelude::*;

const DEFAULT_LOCATION: Option<Url> = None;
const DEFAULT_USER_AGENT: &str = concat!(
    "GStreamer reqwesthttpsrc ",
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("COMMIT_ID")
);
const DEFAULT_IS_LIVE: bool = false;
const DEFAULT_TIMEOUT: u32 = 15;
const DEFAULT_COMPRESS: bool = false;
const DEFAULT_IRADIO_MODE: bool = true;
const DEFAULT_KEEP_ALIVE: bool = true;

#[derive(Debug, Clone)]
struct Settings {
    location: Option<Url>,
    user_agent: String,
    user_id: Option<String>,
    user_pw: Option<String>,
    timeout: u32,
    compress: bool,
    extra_headers: Option<gst::Structure>,
    cookies: Vec<String>,
    iradio_mode: bool,
    keep_alive: bool,
    // Notes about souphttpsrc compatibility:
    // Internal representation of no proxy is None,
    // but externally Some("").
    // Default is set from env var 'http_proxy'.
    // Prepends http:// if not protocol specified.
    proxy: Option<String>,
    // Nullable fields that behave normally:
    proxy_id: Option<String>,
    proxy_pw: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            location: DEFAULT_LOCATION,
            user_agent: DEFAULT_USER_AGENT.into(),
            user_id: None,
            user_pw: None,
            timeout: DEFAULT_TIMEOUT,
            compress: DEFAULT_COMPRESS,
            extra_headers: None,
            cookies: Vec::new(),
            iradio_mode: DEFAULT_IRADIO_MODE,
            keep_alive: DEFAULT_KEEP_ALIVE,
            proxy: proxy_from_str(std::env::var("http_proxy").ok()).unwrap_or_default(),
            proxy_id: None,
            proxy_pw: None,
        }
    }
}

fn proxy_from_str(s: Option<String>) -> Result<Option<String>, glib::Error> {
    match s {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(not_empty_str) => {
            // If no protocol specified, prepend http for compatibility
            // https://gstreamer.freedesktop.org/documentation/soup/souphttpsrc.html
            let url_string = if !not_empty_str.contains("://") {
                format!("http://{not_empty_str}")
            } else {
                not_empty_str
            };
            match reqwest::Url::parse(&url_string) {
                Ok(url) => {
                    // this may urlencode and add trailing /
                    Ok(Some(url.to_string()))
                }
                Err(err) => Err(glib::Error::new(
                    gst::URIError::BadUri,
                    format!("Failed to parse URI '{url_string}': {err:?}").as_str(),
                )),
            }
        }
    }
}

const REQWEST_CLIENT_CONTEXT: &str = "gst.reqwest.client";

#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "GstReqwestClientContext")]
struct ClientContext(Arc<ClientContextInner>);

#[derive(Debug)]
struct ClientContextInner {
    client: Client,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Default)]
enum State {
    #[default]
    Stopped,
    Started {
        uri: Url,
        response: Option<Response>,
        seekable: bool,
        position: u64,
        size: Option<u64>,
        stop: Option<u64>,
        caps: Option<gst::Caps>,
        tags: Option<gst::TagList>,
    },
}

#[derive(Default)]
enum Canceller {
    #[default]
    None,
    Handle(future::AbortHandle),
    Cancelled,
}

impl Canceller {
    fn abort(&mut self) {
        if let Canceller::Handle(ref canceller) = *self {
            canceller.abort();
        }

        *self = Canceller::Cancelled;
    }
}

#[derive(Default)]
pub struct ReqwestHttpSrc {
    client: Mutex<Option<ClientContext>>,
    external_client: Mutex<Option<ClientContext>>,
    settings: Mutex<Settings>,
    state: Mutex<State>,
    canceller: Mutex<Canceller>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "reqwesthttpsrc",
        gst::DebugColorFlags::empty(),
        Some("Rust HTTP source"),
    )
});

static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .unwrap()
});

impl ReqwestHttpSrc {
    fn set_location(&self, uri: Option<&str>) -> Result<(), glib::Error> {
        let state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            return Err(glib::Error::new(
                gst::URIError::BadState,
                "Changing the `location` property on a started `reqwesthttpsrc` is not supported",
            ));
        }

        let mut settings = self.settings.lock().unwrap();

        if uri.is_none() {
            settings.location = DEFAULT_LOCATION;
            return Ok(());
        }

        let uri = uri.unwrap();
        let uri = Url::parse(uri).map_err(|err| {
            glib::Error::new(
                gst::URIError::BadUri,
                format!("Failed to parse URI '{uri}': {err:?}").as_str(),
            )
        })?;

        if uri.scheme() != "http" && uri.scheme() != "https" {
            return Err(glib::Error::new(
                gst::URIError::UnsupportedProtocol,
                format!("Unsupported URI scheme '{}'", uri.scheme()).as_str(),
            ));
        }

        settings.location = Some(uri);

        Ok(())
    }

    /// Set a proxy-related property and perform necessary state checks and modifications to client.
    fn set_proxy_prop<F>(
        &self,
        property_name: &str,
        desired_value: Option<String>,
        prop_memory_location: F,
    ) -> Result<(), glib::Error>
    where
        F: Fn(&mut Settings) -> &mut Option<String>,
    {
        // Proxy props can only be changed when not started.
        let state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            return Err(glib::Error::new(
                gst::URIError::BadState,
                &format!(
                    "Changing the `{property_name}` property on a started `reqwesthttpsrc` is not supported"
                ),
            ));
        }

        // Get memory address of specific variable to change.
        let mut settings = self.settings.lock().unwrap();
        let target_variable = prop_memory_location(&mut settings);
        if &desired_value == target_variable {
            return Ok(());
        }

        // If the Proxy is changed we need to throw away the old client since it isn't properly
        // configured with a proxy anymore. Since element is not started, an existing client
        // without proxy will be used, or a new one with/without proxy will be built on next call
        // to ensure_client.
        *self.client.lock().unwrap() = None;
        *target_variable = desired_value;

        Ok(())
    }

    fn ensure_client(
        &self,
        proxy: Option<String>,
        proxy_id: Option<String>,
        proxy_pw: Option<String>,
    ) -> Result<ClientContext, gst::ErrorMessage> {
        let mut client_guard = self.client.lock().unwrap();
        if let Some(ref client) = *client_guard {
            gst::debug!(CAT, imp = self, "Using already configured client");
            return Ok(client.clone());
        }

        // Attempt to acquire an existing client context from another element instance
        // unless using proxy, because proxy is client specific.
        if proxy.is_none() {
            let mut q = gst::query::Context::new(REQWEST_CLIENT_CONTEXT);
            if self.obj().src_pad().peer_query(&mut q) {
                if let Some(context) = q.context_owned() {
                    self.obj().set_context(&context);
                }
            } else {
                let _ = self.obj().post_message(
                    gst::message::NeedContext::builder(REQWEST_CLIENT_CONTEXT)
                        .src(&*self.obj())
                        .build(),
                );
            }

            // Hopefully now, self.set_context will have been synchronously called
            if let Some(client) = self.external_client.lock().unwrap().clone() {
                gst::debug!(CAT, imp = self, "Using shared client");
                *client_guard = Some(client.clone());

                return Ok(client);
            }
        }

        let mut builder = Client::builder().cookie_store(true).gzip(true);

        if let Some(proxy) = &proxy {
            // Proxy is url-checked on property set but perhaps this might still fail.
            let mut p = reqwest::Proxy::all(proxy).map_err(|err| {
                gst::error_msg!(gst::ResourceError::OpenRead, ["Bad proxy URI: {}", err])
            })?;
            if let Some(proxy_id) = &proxy_id {
                let proxy_pw = proxy_pw.as_deref().unwrap_or("");
                p = p.basic_auth(proxy_id, proxy_pw);
            }
            builder = builder.proxy(p);
        }

        gst::debug!(CAT, imp = self, "Creating new client");
        let client = ClientContext(Arc::new(ClientContextInner {
            client: builder.build().map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to create Client: {}", err]
                )
            })?,
        }));

        // Share created client with other elements, unless using proxy. Shared client never uses proxy.
        // The alternative would be different contexts for different proxy settings, or one context with a
        // map from proxy settings to client, but then, how and when to discard those, retaining reuse benefits?
        if proxy.is_none() {
            gst::debug!(CAT, imp = self, "Sharing new client with other elements");
            let mut context = gst::Context::new(REQWEST_CLIENT_CONTEXT, true);
            {
                let context = context.get_mut().unwrap();
                let s = context.structure_mut();
                s.set("client", &client);
            }
            self.obj().set_context(&context);
            let _ = self.obj().post_message(
                gst::message::HaveContext::builder(context)
                    .src(&*self.obj())
                    .build(),
            );
        }

        *client_guard = Some(client.clone());

        Ok(client)
    }

    fn do_request(
        &self,
        uri: Url,
        start: u64,
        stop: Option<u64>,
    ) -> Result<State, Option<gst::ErrorMessage>> {
        use headers::{Connection, ContentLength, ContentRange, HeaderMapExt, Range, UserAgent};
        use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};

        gst::debug!(CAT, imp = self, "Creating new request for {}", uri);

        let settings = self.settings.lock().unwrap().clone();

        let req = self
            .ensure_client(settings.proxy, settings.proxy_id, settings.proxy_pw)?
            .0
            .client
            .get(uri.clone());

        let mut headers = HeaderMap::new();

        if settings.keep_alive {
            headers.typed_insert(Connection::keep_alive());
        } else {
            headers.typed_insert(Connection::close());
        }

        match (start != 0, stop) {
            (false, None) => (),
            (true, None) => {
                headers.typed_insert(Range::bytes(start..).unwrap());
            }
            (_, Some(stop)) => {
                headers.typed_insert(Range::bytes(start..stop).unwrap());
            }
        }

        headers.typed_insert(settings.user_agent.parse::<UserAgent>().unwrap());

        if !settings.compress {
            // Compression is the default
            headers.insert(
                header::ACCEPT_ENCODING,
                "identity".parse::<HeaderValue>().unwrap(),
            );
        };

        if let Some(ref extra_headers) = settings.extra_headers {
            for (field, value) in extra_headers.iter() {
                let field = match HeaderName::try_from(field.as_str()) {
                    Ok(field) => field,
                    Err(err) => {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Failed to transform extra-header field name '{}' to header name: {}",
                            field,
                            err,
                        );

                        continue;
                    }
                };

                let mut append_header = |field: &HeaderName, value: &glib::Value| {
                    let value = match value.transform::<String>() {
                        Ok(value) => value,
                        Err(_) => {
                            gst::warning!(
                                CAT,
                                imp = self,
                                "Failed to transform extra-header '{}' value to string",
                                field
                            );
                            return;
                        }
                    };

                    let value = value.get::<Option<&str>>().unwrap().unwrap_or("");

                    let value = match value.parse::<HeaderValue>() {
                        Ok(value) => value,
                        Err(_) => {
                            gst::warning!(
                                CAT,
                                imp = self,
                                "Failed to transform extra-header '{}' value to header value",
                                field
                            );
                            return;
                        }
                    };

                    headers.append(field.clone(), value);
                };

                if let Ok(values) = value.get::<gst::ArrayRef>() {
                    for value in values.as_slice() {
                        append_header(&field, value);
                    }
                } else if let Ok(values) = value.get::<gst::ListRef>() {
                    for value in values.as_slice() {
                        append_header(&field, value);
                    }
                } else {
                    append_header(&field, value);
                }
            }
        }

        if !settings.cookies.is_empty() {
            headers.insert(
                header::COOKIE,
                settings.cookies.join("; ").parse::<HeaderValue>().unwrap(),
            );
        }

        if settings.iradio_mode {
            headers.insert("icy-metadata", "1".parse().unwrap());
        }

        // Add all headers for the request here
        let req = req.headers(headers);

        let req = if let Some(ref user_id) = settings.user_id {
            // HTTP auth available
            req.basic_auth(user_id, settings.user_pw)
        } else {
            req
        };

        gst::debug!(CAT, imp = self, "Sending new request: {:?}", req);

        let future = async {
            req.send().await.map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to fetch {}: {:?}", uri, err]
                )
            })
        };
        let res = self.wait(future);

        let res = match res {
            Ok(res) => res,
            Err(Some(err)) => {
                gst::debug!(CAT, imp = self, "Error {:?}", err);
                return Err(Some(err));
            }
            Err(None) => {
                gst::debug!(CAT, imp = self, "Flushing");
                return Err(None);
            }
        };

        gst::debug!(CAT, imp = self, "Received response: {:?}", res);

        if !res.status().is_success() {
            match res.status() {
                StatusCode::NOT_FOUND => {
                    gst::error!(CAT, imp = self, "Resource not found");
                    return Err(Some(gst::error_msg!(
                        gst::ResourceError::NotFound,
                        ["Resource '{}' not found", uri]
                    )));
                }
                StatusCode::UNAUTHORIZED
                | StatusCode::PAYMENT_REQUIRED
                | StatusCode::FORBIDDEN
                | StatusCode::PROXY_AUTHENTICATION_REQUIRED => {
                    gst::error!(CAT, imp = self, "Not authorized: {}", res.status());
                    return Err(Some(gst::error_msg!(
                        gst::ResourceError::NotAuthorized,
                        ["Not Authorized for resource '{}': {}", uri, res.status()]
                    )));
                }
                _ => {
                    gst::error!(CAT, imp = self, "Request failed: {}", res.status());
                    return Err(Some(gst::error_msg!(
                        gst::ResourceError::OpenRead,
                        ["Request for '{}' failed: {}", uri, res.status()]
                    )));
                }
            }
        }

        let headers = res.headers();
        let size = headers
            .typed_get::<ContentLength>()
            .map(|ContentLength(cl)| cl + start);

        let accept_byte_ranges = headers
            .get(header::ACCEPT_RANGES)
            .map(|ranges| ranges == "bytes")
            .unwrap_or(false);
        let seekable = size.is_some() && accept_byte_ranges;

        #[allow(clippy::manual_unwrap_or_default)]
        // https://github.com/rust-lang/rust-clippy/issues/12928
        let position = if let Some((range_start, _)) = headers
            .typed_get::<ContentRange>()
            .and_then(|range| range.bytes_range())
        {
            range_start
        } else {
            0
        };

        if position != start {
            return Err(Some(gst::error_msg!(
                gst::ResourceError::Seek,
                ["Failed to seek to {}: Got {}", start, position]
            )));
        }

        let mut caps = headers
            .get("icy-metaint")
            .and_then(|s| s.to_str().ok())
            .and_then(|s| s.parse::<i32>().ok())
            .map(|icy_metaint| {
                gst::Caps::builder("application/x-icy")
                    .field("metadata-interval", icy_metaint)
                    .build()
            });

        if let Some(content_type) = headers
            .get(header::CONTENT_TYPE)
            .and_then(|content_type| content_type.to_str().ok())
            .and_then(|content_type| content_type.parse::<mime::Mime>().ok())
        {
            gst::debug!(CAT, imp = self, "Got content type {}", content_type);
            if let Some(ref mut caps) = caps {
                let caps = caps.get_mut().unwrap();
                let s = caps.structure_mut(0).unwrap();
                s.set("content-type", content_type.as_ref());
            } else if content_type.type_() == "audio" && content_type.subtype() == "L16" {
                let channels = content_type
                    .get_param("channels")
                    .and_then(|s| s.as_ref().parse::<i32>().ok())
                    .unwrap_or(2);
                let rate = content_type
                    .get_param("rate")
                    .and_then(|s| s.as_ref().parse::<i32>().ok())
                    .unwrap_or(44_100);

                caps = Some(
                    gst::Caps::builder("audio/x-unaligned-raw")
                        .field("format", "S16BE")
                        .field("layout", "interleaved")
                        .field("channels", channels)
                        .field("rate", rate)
                        .build(),
                );
            }
        }

        let mut tags = gst::TagList::new();
        {
            let tags = tags.get_mut().unwrap();

            if let Some(ref icy_name) = headers.get("icy-name").and_then(|s| s.to_str().ok()) {
                tags.add::<gst::tags::Organization>(icy_name, gst::TagMergeMode::Replace);
            }

            if let Some(ref icy_genre) = headers.get("icy-genre").and_then(|s| s.to_str().ok()) {
                tags.add::<gst::tags::Genre>(icy_genre, gst::TagMergeMode::Replace);
            }

            if let Some(ref icy_url) = headers.get("icy-url").and_then(|s| s.to_str().ok()) {
                tags.add::<gst::tags::Location>(icy_url, gst::TagMergeMode::Replace);
            }
        }

        gst::debug!(CAT, imp = self, "Request successful");

        Ok(State::Started {
            uri,
            response: Some(res),
            seekable,
            position,
            size,
            stop,
            caps,
            tags: if tags.n_tags() > 0 { Some(tags) } else { None },
        })
    }

    fn wait<F, T>(&self, future: F) -> Result<T, Option<gst::ErrorMessage>>
    where
        F: Send + Future<Output = Result<T, gst::ErrorMessage>>,
        T: Send + 'static,
    {
        let timeout = self.settings.lock().unwrap().timeout;

        let mut canceller = self.canceller.lock().unwrap();
        if matches!(*canceller, Canceller::Cancelled) {
            return Err(None);
        }
        let (abort_handle, abort_registration) = future::AbortHandle::new_pair();
        *canceller = Canceller::Handle(abort_handle);
        drop(canceller);

        // Wrap in a timeout
        let future = async {
            if timeout == 0 {
                future.await
            } else {
                let res = tokio::time::timeout(Duration::from_secs(timeout.into()), future).await;

                match res {
                    Ok(res) => res,
                    Err(_) => Err(gst::error_msg!(
                        gst::ResourceError::Read,
                        ["Request timeout"]
                    )),
                }
            }
        };

        // And make abortable
        let future = async {
            match future::Abortable::new(future, abort_registration).await {
                Ok(res) => res.map_err(Some),
                Err(_) => Err(None),
            }
        };

        let res = {
            let _enter = RUNTIME.enter();
            futures::executor::block_on(future)
        };

        /* Clear out the canceller */
        let mut canceller = self.canceller.lock().unwrap();
        if matches!(*canceller, Canceller::Cancelled) {
            return Err(None);
        }
        *canceller = Canceller::None;

        res
    }
}

impl ObjectImpl for ReqwestHttpSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            #[allow(unused_mut)]
            let mut user_agent_pspec = glib::ParamSpecString::builder("user-agent")
                .nick("User-Agent")
                .blurb("Value of the User-Agent HTTP request header field")
                .default_value("GStreamer reqwesthttpsrc")
                .readwrite()
                .readwrite()
                .mutable_ready();

            #[cfg(feature = "doc")]
            {
                user_agent_pspec = user_agent_pspec.doc_show_default();
            }

            vec![
                glib::ParamSpecString::builder("location")
                    .nick("Location")
                    .blurb("URL to read from")
                    .readwrite()
                    .mutable_ready()
                    .build(),
                user_agent_pspec.build(),
                glib::ParamSpecBoolean::builder("is-live")
                    .nick("Is Live")
                    .blurb("Act like a live source")
                    .default_value(DEFAULT_IS_LIVE)
                    .readwrite()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("user-id")
                    .nick("User-id")
                    .blurb("HTTP location URI user id for authentication")
                    .readwrite()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("user-pw")
                    .nick("User-pw")
                    .blurb("HTTP location URI user password for authentication")
                    .readwrite()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("timeout")
                    .nick("Timeout")
                    .blurb("Value in seconds to timeout a blocking I/O (0 = No timeout).")
                    .maximum(3600)
                    .default_value(DEFAULT_TIMEOUT)
                    .readwrite()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("compress")
                    .nick("Compress")
                    .blurb("Allow compressed content encodings")
                    .default_value(DEFAULT_COMPRESS)
                    .readwrite()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("extra-headers")
                    .nick("Extra Headers")
                    .blurb("Extra headers to append to the HTTP request")
                    .readwrite()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<Vec<String>>("cookies")
                    .nick("Cookies")
                    .nick("HTTP request cookies")
                    .readwrite()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("iradio-mode")
                    .nick("I-Radio Mode")
                    .blurb("Enable internet radio mode (ask server to send shoutcast/icecast metadata interleaved with the actual stream data")
                    .default_value(DEFAULT_IRADIO_MODE)
                    .readwrite()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("keep-alive")
                    .nick("Keep Alive")
                    .blurb("Use HTTP persistent connections")
                    .default_value(DEFAULT_KEEP_ALIVE)
                    .readwrite()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("proxy")
                    .nick("Proxy")
                    .blurb("HTTP proxy server URI")
                    .default_value(Some(""))
                    .readwrite()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("proxy-id")
                    .nick("Proxy-id")
                    .blurb("HTTP proxy URI user id for authentication")
                    .default_value(Some(""))
                    .readwrite()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("proxy-pw")
                    .nick("Proxy-pw")
                    .blurb("HTTP proxy URI user password for authentication")
                    .default_value(Some(""))
                    .readwrite()
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let res = match pspec.name() {
            "location" => {
                let location = value.get::<Option<&str>>().expect("type checked upstream");
                self.set_location(location)
            }
            "user-agent" => {
                let mut settings = self.settings.lock().unwrap();
                let user_agent = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_USER_AGENT.into());
                settings.user_agent = user_agent;
                Ok(())
            }
            "is-live" => {
                let is_live = value.get().expect("type checked upstream");
                self.obj().set_live(is_live);
                Ok(())
            }
            "user-id" => {
                let mut settings = self.settings.lock().unwrap();
                let user_id = value.get().expect("type checked upstream");
                settings.user_id = user_id;
                Ok(())
            }
            "user-pw" => {
                let mut settings = self.settings.lock().unwrap();
                let user_pw = value.get().expect("type checked upstream");
                settings.user_pw = user_pw;
                Ok(())
            }
            "timeout" => {
                let mut settings = self.settings.lock().unwrap();
                let timeout = value.get().expect("type checked upstream");
                settings.timeout = timeout;
                Ok(())
            }
            "compress" => {
                let mut settings = self.settings.lock().unwrap();
                let compress = value.get().expect("type checked upstream");
                settings.compress = compress;
                Ok(())
            }
            "extra-headers" => {
                let mut settings = self.settings.lock().unwrap();
                let extra_headers = value.get().expect("type checked upstream");
                settings.extra_headers = extra_headers;
                Ok(())
            }
            "cookies" => {
                let mut settings = self.settings.lock().unwrap();
                settings.cookies = value.get::<Vec<String>>().expect("type checked upstream");
                Ok(())
            }
            "iradio-mode" => {
                let mut settings = self.settings.lock().unwrap();
                let iradio_mode = value.get().expect("type checked upstream");
                settings.iradio_mode = iradio_mode;
                Ok(())
            }
            "keep-alive" => {
                let mut settings = self.settings.lock().unwrap();
                let keep_alive = value.get().expect("type checked upstream");
                settings.keep_alive = keep_alive;
                Ok(())
            }
            "proxy" => {
                let proxy = proxy_from_str(
                    value
                        .get::<Option<String>>()
                        .expect("type checked upstream"),
                );
                match proxy {
                    Ok(proxy) => self
                        .set_proxy_prop(pspec.name(), proxy, move |settings| &mut settings.proxy),
                    Err(e) => Err(e),
                }
            }
            "proxy-id" => {
                let proxy_id = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
                self.set_proxy_prop(pspec.name(), proxy_id, move |settings| {
                    &mut settings.proxy_id
                })
            }
            "proxy-pw" => {
                let proxy_pw = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
                self.set_proxy_prop(pspec.name(), proxy_pw, move |settings| {
                    &mut settings.proxy_pw
                })
            }
            _ => unimplemented!(),
        };

        if let Err(err) = res {
            gst::error!(
                CAT,
                imp = self,
                "Failed to set property `{}`: {:?}",
                pspec.name(),
                err
            );
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "location" => {
                let settings = self.settings.lock().unwrap();
                let location = settings.location.as_ref().map(Url::to_string);

                location.to_value()
            }
            "user-agent" => {
                let settings = self.settings.lock().unwrap();
                settings.user_agent.to_value()
            }
            "is-live" => self.obj().is_live().to_value(),
            "user-id" => {
                let settings = self.settings.lock().unwrap();
                settings.user_id.to_value()
            }
            "user-pw" => {
                let settings = self.settings.lock().unwrap();
                settings.user_pw.to_value()
            }
            "timeout" => {
                let settings = self.settings.lock().unwrap();
                settings.timeout.to_value()
            }
            "compress" => {
                let settings = self.settings.lock().unwrap();
                settings.compress.to_value()
            }
            "extra-headers" => {
                let settings = self.settings.lock().unwrap();
                settings.extra_headers.to_value()
            }
            "cookies" => {
                let settings = self.settings.lock().unwrap();
                settings.cookies.to_value()
            }
            "iradio-mode" => {
                let settings = self.settings.lock().unwrap();
                settings.iradio_mode.to_value()
            }
            "keep-alive" => {
                let settings = self.settings.lock().unwrap();
                settings.keep_alive.to_value()
            }
            // return None values as Some("") for compatibility with souphttpsrc
            "proxy" => self
                .settings
                .lock()
                .unwrap()
                .proxy
                .as_deref()
                .unwrap_or("")
                .to_value(),
            "proxy-id" => self.settings.lock().unwrap().proxy_id.to_value(),
            "proxy-pw" => self.settings.lock().unwrap().proxy_pw.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_automatic_eos(false);
        obj.set_format(gst::Format::Bytes);
    }
}

impl GstObjectImpl for ReqwestHttpSrc {}

impl ElementImpl for ReqwestHttpSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "HTTP Source",
                "Source/Network/HTTP",
                "Read stream from an HTTP/HTTPS location",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn set_context(&self, context: &gst::Context) {
        if context.context_type() == REQWEST_CLIENT_CONTEXT {
            let mut external_client = self.external_client.lock().unwrap();
            let s = context.structure();
            *external_client = s
                .get::<&ClientContext>("client")
                .map(|c| Some(c.clone()))
                .unwrap_or(None);
        }

        self.parent_set_context(context);
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        if let gst::StateChange::ReadyToNull = transition {
            *self.client.lock().unwrap() = None;
        }

        self.parent_change_state(transition)
    }
}

impl BaseSrcImpl for ReqwestHttpSrc {
    fn is_seekable(&self) -> bool {
        let state = self.state.lock().unwrap();
        match *state {
            State::Started { seekable, .. } => seekable,
            _ => false,
        }
    }

    fn size(&self) -> Option<u64> {
        let state = self.state.lock().unwrap();
        match *state {
            State::Started { size, .. } => size,
            _ => None,
        }
    }

    fn unlock(&self) -> Result<(), gst::ErrorMessage> {
        let mut canceller = self.canceller.lock().unwrap();
        canceller.abort();
        Ok(())
    }

    fn unlock_stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut canceller = self.canceller.lock().unwrap();
        *canceller = Canceller::None;
        Ok(())
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();

        *state = State::Stopped;

        let uri = self
            .settings
            .lock()
            .unwrap()
            .location
            .as_ref()
            .ok_or_else(|| {
                gst::error_msg!(gst::CoreError::StateChange, ["Can't start without an URI"])
            })
            .cloned()?;

        gst::debug!(CAT, imp = self, "Starting for URI {}", uri);

        *state = self.do_request(uri, 0, None).map_err(|err| {
            err.unwrap_or_else(|| {
                gst::error_msg!(gst::LibraryError::Failed, ["Interrupted during start"])
            })
        })?;

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Stopping");
        *self.state.lock().unwrap() = State::Stopped;

        Ok(())
    }

    fn query(&self, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        match query.view_mut() {
            QueryViewMut::Scheduling(q) => {
                q.set(
                    gst::SchedulingFlags::SEQUENTIAL | gst::SchedulingFlags::BANDWIDTH_LIMITED,
                    1,
                    -1,
                    0,
                );
                q.add_scheduling_modes([gst::PadMode::Push]);
                true
            }
            _ => BaseSrcImplExt::parent_query(self, query),
        }
    }

    fn do_seek(&self, segment: &mut gst::Segment) -> bool {
        let segment = segment.downcast_mut::<gst::format::Bytes>().unwrap();

        let mut state = self.state.lock().unwrap();

        let (position, old_stop, uri) = match *state {
            State::Started {
                position,
                stop,
                ref uri,
                ..
            } => (position, stop, uri.clone()),
            State::Stopped => {
                gst::element_imp_error!(self, gst::LibraryError::Failed, ["Not started yet"]);

                return false;
            }
        };

        let start = *segment.start().expect("No start position given");
        let stop = segment.stop().map(|stop| *stop);

        gst::debug!(CAT, imp = self, "Seeking to {}-{:?}", start, stop);

        if position == start && old_stop == stop {
            gst::debug!(CAT, imp = self, "No change to current request");
            return true;
        }

        *state = State::Stopped;
        match self.do_request(uri, start, stop) {
            Ok(s) => {
                *state = s;
                true
            }
            Err(Some(err)) => {
                self.post_error_message(err);
                false
            }
            Err(None) => false,
        }
    }
}

impl PushSrcImpl for ReqwestHttpSrc {
    fn create(
        &self,
        _buffer: Option<&mut gst::BufferRef>,
    ) -> Result<CreateSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();

        let (response, position, caps, tags) = match *state {
            State::Started {
                ref mut response,
                ref mut position,
                ref mut tags,
                ref mut caps,
                ..
            } => (response, position, caps, tags),
            State::Stopped => {
                gst::element_imp_error!(self, gst::LibraryError::Failed, ["Not started yet"]);

                return Err(gst::FlowError::Error);
            }
        };

        let offset = *position;

        let mut current_response = match response.take() {
            Some(response) => response,
            None => {
                gst::error!(CAT, imp = self, "Don't have a response");
                gst::element_imp_error!(self, gst::ResourceError::Read, ["Don't have a response"]);

                return Err(gst::FlowError::Error);
            }
        };

        let tags = tags.take();
        let caps = caps.take();
        drop(state);

        if let Some(caps) = caps {
            gst::debug!(CAT, imp = self, "Setting caps {:?}", caps);
            self.obj()
                .set_caps(&caps)
                .map_err(|_| gst::FlowError::NotNegotiated)?;
        }

        if let Some(tags) = tags {
            gst::debug!(CAT, imp = self, "Sending iradio tags {:?}", tags);
            self.obj().src_pad().push_event(gst::event::Tag::new(tags));
        }

        let future = async {
            current_response.chunk().await.map_err(move |err| {
                gst::error_msg!(
                    gst::ResourceError::Read,
                    ["Failed to read chunk at offset {}: {:?}", offset, err]
                )
            })
        };
        let res = self.wait(future);

        let res = match res {
            Ok(res) => res,
            Err(Some(err)) => {
                gst::debug!(CAT, imp = self, "Error {:?}", err);
                self.post_error_message(err);
                return Err(gst::FlowError::Error);
            }
            Err(None) => {
                gst::debug!(CAT, imp = self, "Flushing");
                return Err(gst::FlowError::Flushing);
            }
        };

        let mut state = self.state.lock().unwrap();
        let (response, position) = match *state {
            State::Started {
                ref mut response,
                ref mut position,
                ..
            } => (response, position),
            State::Stopped => {
                gst::element_imp_error!(self, gst::LibraryError::Failed, ["Not started yet"]);

                return Err(gst::FlowError::Error);
            }
        };

        match res {
            Some(chunk) => {
                /* do something with the chunk and store the body again in the state */

                gst::trace!(
                    CAT,
                    imp = self,
                    "Chunk of {} bytes received at offset {}",
                    chunk.len(),
                    offset
                );
                let size = chunk.len();
                assert_ne!(chunk.len(), 0);

                *position += size as u64;

                let mut buffer = gst::Buffer::from_slice(chunk);

                *response = Some(current_response);

                {
                    let buffer = buffer.get_mut().unwrap();
                    buffer.set_offset(offset);
                    buffer.set_offset_end(offset + size as u64);
                }

                Ok(CreateSuccess::NewBuffer(buffer))
            }
            None => {
                /* No further data, end of stream */
                gst::debug!(CAT, imp = self, "End of stream");
                *response = Some(current_response);
                Err(gst::FlowError::Eos)
            }
        }
    }
}

impl URIHandlerImpl for ReqwestHttpSrc {
    const URI_TYPE: gst::URIType = gst::URIType::Src;

    fn protocols() -> &'static [&'static str] {
        &["http", "https"]
    }

    fn uri(&self) -> Option<String> {
        let settings = self.settings.lock().unwrap();

        settings.location.as_ref().map(Url::to_string)
    }

    fn set_uri(&self, uri: &str) -> Result<(), glib::Error> {
        self.set_location(Some(uri))
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ReqwestHttpSrc {
    const NAME: &'static str = "GstReqwestHttpSrc";
    type Type = super::ReqwestHttpSrc;
    type ParentType = gst_base::PushSrc;
    type Interfaces = (gst::URIHandler,);
}

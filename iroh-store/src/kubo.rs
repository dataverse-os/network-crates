use ceramic_kubo_rpc_server::{ApiNoContext, ContextWrapperExt};
use swagger::{AuthData, ContextBuilder, EmptyContext, Push, XSpanIdString};

pub type ClientContext = swagger::make_context_ty!(
    ContextBuilder,
    EmptyContext,
    Option<AuthData>,
    XSpanIdString
);

pub type Client = Box<dyn ApiNoContext<ClientContext> + Send + Sync>;

pub fn new(base_path: &str) -> Client {
    let context: ClientContext = swagger::make_context!(
        ContextBuilder,
        EmptyContext,
        None as Option<AuthData>,
        XSpanIdString::default()
    );

    // Using HTTP
    let client = Box::new(
        ceramic_kubo_rpc_server::Client::try_new_http(&base_path)
            .expect("Failed to create HTTP client"),
    );
    Box::new(client.with_context(context))
}

//! JSON shapes of the OpenAI Responses API (`/v1/responses`). Private to the
//! adapter: the rest of the code only sees the neutral types.

mod input_item;
mod model;
mod output_item;
mod responses_request;
mod responses_response;
mod wire_tool;
mod wire_usage;

pub(crate) use model::ModelInfo;
pub(crate) use responses_request::ResponsesRequest;
pub(crate) use responses_response::ResponsesResponse;

/// The provider name stored in `ContentBlock::ProviderState` for OpenAI state.
pub(crate) const PROVIDER: &str = "openai";

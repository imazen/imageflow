pub(crate) mod endpoints;
use crate::context::Context;
use crate::internal_prelude::works_everywhere::*;

pub(crate) fn invoke_with_json_error(
    context: &mut Context,
    endpoint: &str,
    json: &[u8],
) -> (JsonResponse, Result<()>) {
    match endpoints::invoke(context, endpoint, json) {
        Ok(response) => (response, Ok(())),
        Err(e) => (JsonResponse::from_flow_error(&e), Err(e)),
    }
}

pub fn invoke(context: &mut Context, endpoint: &str, json: &[u8]) -> Result<JsonResponse> {
    endpoints::invoke(context, endpoint, json)
}

pub fn try_invoke_static(endpoint: &str, json: &[u8]) -> Result<Option<JsonResponse>> {
    endpoints::try_invoke_static(endpoint, json).map_err(|e| e.at(here!()))
}

pub(crate) fn parse_json<'a, D>(json: &[u8]) -> Result<D>
where
    D: serde::de::DeserializeOwned,
    D: 'a,
{
    match serde_json::from_slice(json) {
        Ok(d) => Ok(d),
        Err(e) => Err(FlowError::from_serde(e, json, std::any::type_name::<D>()).at(here!())),
    }
}

#[cfg(feature = "schema-export")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "schema-export")]
use utoipa::ToSchema;

// Generic wrapper for successful JSON responses (matches Response001 structure)
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonAnswer<T> {
    #[cfg_attr(feature = "schema-export", schema(example = 200))]
    pub code: i64,
    #[cfg_attr(feature = "schema-export", schema(example = true))]
    pub success: bool,
    pub message: Option<String>,
    pub data: T, // Specific payload for the endpoint
}

#[derive(Debug, Clone)]
pub struct JsonResponse {
    pub status_code: i64,
    pub response_json: Cow<'static, [u8]>,
}

impl JsonResponse {
    pub fn from_flow_error(err: &FlowError) -> JsonResponse {
        let message = format!("{}", err);
        JsonResponse::fail_with_message(i64::from(err.category().http_status_code()), &message)
    }

    pub fn from_panic(err: &Box<dyn std::any::Any>) -> JsonResponse {
        let message = format!("{:#?}", err);
        JsonResponse::fail_with_message(500, &message)
    }

    pub fn from_response001(r: s::Response001) -> JsonResponse {
        JsonResponse {
            status_code: 400,
            response_json: Cow::Owned(serde_json::to_vec_pretty(&r).unwrap()),
        }
    }
    pub fn success_with_payload(r: s::ResponsePayload) -> JsonResponse {
        let r =
            s::Response001 { success: true, code: 200, message: Some("OK".to_owned()), data: r };
        JsonResponse {
            status_code: r.code,
            response_json: Cow::Owned(serde_json::to_vec_pretty(&r).unwrap()),
        }
    }
    pub fn ok<T>(r: T) -> JsonResponse
    where
        T: serde::Serialize,
    {
        let r = JsonAnswer { success: true, code: 200, message: Some("OK".to_owned()), data: r };
        JsonResponse {
            status_code: r.code,
            response_json: Cow::Owned(serde_json::to_vec_pretty(&r).unwrap()),
        }
    }
    pub fn from_result(r: Result<s::ResponsePayload>) -> JsonResponse {
        match r {
            Ok(payload) => {
                JsonResponse::success_with_payload(payload) //How about failures with payloads!?
            }
            Err(error) => JsonResponse::from_flow_error(&error),
        }
    }

    pub fn status_2xx(&self) -> bool {
        self.status_code >= 200 && self.status_code < 300
    }
    pub fn assert_ok(&self) {
        if !self.status_2xx() {
            if let Ok(s) = std::str::from_utf8(self.response_json.as_ref()) {
                if let Ok(s::Response001 { message: Some(message), .. }) =
                    serde_json::from_slice(self.response_json.as_ref())
                {
                    panic!("Json Status {}\n{}\n{}", self.status_code, &s, message);
                }
                panic!("Json Status {}\n{}", self.status_code, &s);
            } else {
                panic!("Json Status {} - payload invalid utf8", self.status_code);
            }
        }
    }
    pub fn unwrap_status200(&self) -> &JsonResponse {
        self.assert_ok();
        self
    }

    pub fn ok_empty() -> JsonResponse {
        JsonResponse {
            status_code: 200,
            response_json: Cow::Borrowed(br#"{"success": "true","code": 200,"message": "OK"}"#),
        }
    }
    pub fn teapot() -> JsonResponse {
        JsonResponse {
            status_code: 418,
            response_json: /* HTTP 418 I'm a teapot per RFC 2324 */
            Cow::Borrowed(br#"{"success": "false","code": 418, "message": "I'm a little teapot, short and stout..."}"#)
        }
    }
    pub fn method_not_understood() -> JsonResponse {
        JsonResponse {
            status_code: 404,
            response_json: Cow::Borrowed(
                br#"{
                                        "success": "false",
                                        "code": 404,
                                        "message": "Endpoint name not understood"}"#,
            ),
        }
    }

    pub fn fail_with_message(code: i64, message: &str) -> JsonResponse {
        let r = s::Response001 {
            success: false,
            code,
            message: Some(message.to_owned()),
            data: s::ResponsePayload::None,
        };
        JsonResponse {
            status_code: r.code,
            response_json: Cow::Owned(serde_json::to_vec_pretty(&r).unwrap()),
        }
    }
}

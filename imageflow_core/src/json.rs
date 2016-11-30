use ::internal_prelude::works_everywhere::*;

type ResponderFn<'a, T, D> = Box<Fn(&mut T, D) -> Result<s::ResponsePayload> + 'a + Sync>;
type MethodHandler<'a, T> = Box<Fn(&mut T, &[u8]) -> Result<JsonResponse> + 'a + Sync>;


pub struct MethodRouter<'a, T> {
    handlers: HashMap<&'static str, MethodHandler<'a, T>>,
    method_names: Vec<&'static str>,
}

impl<'a, T> MethodRouter<'a, T> {
    pub fn new() -> MethodRouter<'a, T> {
        MethodRouter {
            handlers: HashMap::new(),
            method_names: vec![],
        }
    }
    /// Returns the replaced MethodHandler if one already existed for that method.
    pub fn add(&mut self,
               method: &'static str,
               handler: MethodHandler<'a, T>)
               -> Option<MethodHandler<'a, T>> {
        self.method_names.push(method);
        self.handlers.insert(method, handler)
    }

    pub fn add_responder<D>(&mut self,
                            method: &'static str,
                            responder: ResponderFn<'a, T, D>)
                            -> Option<MethodHandler<'a, T>>
        where D: serde::Deserialize,
              D: 'a,
              T: 'a
    {
        self.method_names.push(method);
        self.handlers.insert(method, create_handler_over_responder(responder))
    }


    pub fn list(&self) -> &[&str] {
        &self.method_names
    }

    /// Responds with an JsonResponse even for client errors
    ///
    pub fn invoke(&self,
                  upon: &mut T,
                  method: &str,
                  json_request_body: &[u8])
                  -> Result<JsonResponse> {
        match self.handlers.get(method) {
            Some(handler) => handler(upon as &mut T, json_request_body),
            None => Ok(JsonResponse::method_not_understood()),
        }
    }
}
pub fn create_handler_over_responder<'a, T, D>(responder: ResponderFn<'a, T, D>)
                                               -> MethodHandler<'a, T>
    where D: serde::Deserialize,
          D: 'a,
          T: 'a
{
    Box::new(move |upon: &mut T, json_request_bytes: &[u8]| {

        let parsed_maybe: std::result::Result<D, serde_json::Error> =
            serde_json::from_slice(json_request_bytes);
        match parsed_maybe {
            Ok(parsed) => {
                let payload_maybe = responder(upon, parsed);
                match payload_maybe {
                    Ok(payload) => {
                        Ok(JsonResponse::success_with_payload(payload)) //How about failures with payloads!?
                    }
                    Err(error) => {
                        let message = format!("{:?}", error);
                        Ok(JsonResponse::fail_with_message(500,
                                                           &message))
                    }
                }
            }
            Err(e) => Ok(JsonResponse::from_parse_error(e, json_request_bytes)),
        }

    })

}


pub struct JsonResponse {
    pub status_code: i64,
    pub response_json: Cow<'static, [u8]>,
}

impl JsonResponse {
    pub fn from_parse_error(err: serde_json::error::Error, json: &[u8]) -> JsonResponse {

        let message = format!("Parse error: {}\n Received {}",
                              err,
                              std::str::from_utf8(json).unwrap_or("[INVALID UTF-8]"));

        let r = s::Response001 {
            success: false,
            code: 400,
            message: Some(message.to_owned()),
            data: s::ResponsePayload::None,
        };
        JsonResponse::from_response001(r)
    }
    pub fn from_response001(r: s::Response001) -> JsonResponse {
        JsonResponse {
            status_code: 400,
            response_json: Cow::Owned(serde_json::to_vec_pretty(&r).unwrap()),
        }
    }
    pub fn success_with_payload(r: s::ResponsePayload) -> JsonResponse {
        let r = s::Response001 {
            success: true,
            code: 200,
            message: Some("OK".to_owned()),
            data: r,
        };
        JsonResponse {
            status_code: r.code,
            response_json: Cow::Owned(serde_json::to_vec_pretty(&r).unwrap()),
        }
    }

    pub fn status_2xx(&self) -> bool {
        self.status_code >= 200 && self.status_code < 300
    }
    pub fn assert_ok(&self) {
        if !self.status_2xx() {
            panic!("status {} - {:?}",
                   self.status_code,
                   std::str::from_utf8(self.response_json.as_ref()).unwrap());
        }
    }
    pub fn unwrap_status200(&self) -> &JsonResponse {
        self.assert_ok();
        self
    }

    pub fn ok() -> JsonResponse {
        JsonResponse {
            status_code: 200,
            response_json: Cow::Borrowed(r#"{"success": "true","code": 200,"message": "OK"}"#
                .as_bytes()),
        }
    }
    pub fn teapot() -> JsonResponse {
        JsonResponse {
            status_code: 418,
            response_json: /* HTTP 418 I'm a teapot per RFC 2324 */
            Cow::Borrowed(r#"{"success": "false","code": 418, "message": "I'm a little teapot, short and stout..."}"#
                .as_bytes())
        }
    }
    pub fn method_not_understood() -> JsonResponse {
        JsonResponse {
            status_code: 404,
            response_json: Cow::Borrowed(r#"{
                                        "success": "false",
                                        "code": 404,
                                        "message": "Endpoint name not understood"}"#
                .as_bytes()),
        }
    }

    pub fn fail_with_message(code: i64, message: &str) -> JsonResponse {
        JsonResponse {
            status_code: 404,
            response_json:
                Cow::Owned(format!("{}\"success\": \"false\",\"code\": {},\"message\": {:?}{}",
                                   "{",
                                   code,
                                   message,
                                   "}")
                .into_bytes()),
        }
    }
}



// struct Meh{}
//
// #[derive(Deserialize,Clone)]
// struct Val{
//    i:i32
// }
//
// fn tryit(){
//    let mut m = Meh{};
//    let mut r = MethodRouter::new();
//    r.add("/api", create_handler_over_responder(
//       Box::new( move |upon: &mut Meh, v: Val|{
//           Ok(s::ResponsePayload::None)
//       })
//    ));
// }

// pub fn wrap<T,D>(responder: Box<Fn(&mut T, D) -> Vec<u8>>) -> Box<Fn(&mut T, &[u8]) -> Vec<u8>> where D: serde::Deserialize, D: 'static{
//    Box::new( move | upon: &mut T, json_request_bytes: & [u8] | {
//        responder(upon, serde_json::from_slice(json_request_bytes).unwrap())
//    })
// }
//

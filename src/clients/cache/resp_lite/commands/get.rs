use super::*;

impl From<&workload::client::Get> for RequestWithValidator {
    fn from(other: &workload::client::Get) -> Self {
        GET.increment();
        RequestWithValidator {
            request: Some(Request::get(&other.key)),
            message: None,
            validator: Box::new(validate_get_response),
        }
    }
}

fn validate_get_response(response: Response) -> std::result::Result<(), ()> {
    match response {
        Response::BulkString(data) => {
            if data.bytes().is_none() {
                // Null bulk string means miss
                RESPONSE_MISS.increment();
                GET_KEY_MISS.increment();
            } else {
                RESPONSE_HIT.increment();
                GET_KEY_HIT.increment();
            }
            Ok(())
        }
        Response::Error(_) => {
            GET_EX.increment();
            Err(())
        }
        _ => {
            GET_EX.increment();
            Err(())
        }
    }
}

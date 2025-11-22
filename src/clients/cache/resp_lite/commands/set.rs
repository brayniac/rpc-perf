use super::*;

impl From<&workload::client::Set> for RequestWithValidator {
    fn from(other: &workload::client::Set) -> Self {
        SET.increment();

        let expire_time = other.ttl.map(|ttl| ExpireTime::Seconds(ttl.as_secs()));

        RequestWithValidator {
            request: Some(Request::set(
                &other.key,
                &other.value,
                expire_time,
                SetMode::Set,
                false,
            )),
            message: None,
            validator: Box::new(validate_set_response),
        }
    }
}

fn validate_set_response(response: Response) -> std::result::Result<(), ()> {
    match response {
        Response::SimpleString(s) if s.as_ref() == "OK" => {
            SET_STORED.increment();
            Ok(())
        }
        Response::BulkString(data) if data.bytes().is_none() => {
            // SET with NX/XX can return null if condition not met
            SET_NOT_STORED.increment();
            Ok(())
        }
        Response::Error(_) => {
            SET_EX.increment();
            Err(())
        }
        _ => {
            SET_EX.increment();
            Err(())
        }
    }
}

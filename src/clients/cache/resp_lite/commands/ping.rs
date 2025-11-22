use super::*;

// Raw PING command bytes: *1\r\n$4\r\nPING\r\n
const PING_COMMAND: &[u8] = b"*1\r\n$4\r\nPING\r\n";

impl RequestWithValidator {
    pub fn ping() -> Self {
        PING.increment();

        RequestWithValidator {
            request: None,
            message: None,  // We'll handle PING specially
            validator: Box::new(validate_ping_response),
        }
    }

    pub fn is_ping(&self) -> bool {
        self.request.is_none() && self.message.is_none()
    }

    pub fn ping_bytes() -> &'static [u8] {
        PING_COMMAND
    }
}

fn validate_ping_response(response: Response) -> std::result::Result<(), ()> {
    match response {
        Response::SimpleString(s) if s.as_ref() == "PONG" => {
            Ok(())
        }
        Response::Error(_) => {
            PING_EX.increment();
            Err(())
        }
        _ => {
            PING_EX.increment();
            Err(())
        }
    }
}

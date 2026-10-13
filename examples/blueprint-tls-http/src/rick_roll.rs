//! Rick-Roll as a service

use blueprint_h11spec::Resp2xx;
use blueprint_h11spec::{H11Header, H11MaybeValue};
use blueprint_h11spec::{H11Method, H11Version};
use blueprint_h11spec::{
    RequestBodyReceiver, RequestHeaderReceiver, RequestReceiver, RequestTargetReceiver,
};
use blueprint_h11spec::{RespComplete, RespIndicative};
use blueprint_h11spec::{ResponseBodySize, ResponseSender};

const RICK_ROLL: &[u8] = core::include_bytes!("/tmp/rick-roll.gif");

/// sus
pub struct RickRoll {
    payload_out_cursor: PayloadCursor,
    headers: [H11Header<'static>; 1],
}

/// Stateful cursor for sending / receiving bytes
#[derive(Debug, Default)]
pub struct PayloadCursor {
    consumed: usize,
}

impl Default for RickRoll {
    fn default() -> Self {
        RickRoll {
            payload_out_cursor: PayloadCursor::default(),
            headers: [H11Header::ContentLength(H11MaybeValue::Integer(
                "193274".as_bytes(),
            ))],
        }
    }
}

impl RequestReceiver for RickRoll {
    /// Provides request method
    fn req_method(&mut self, mthd: H11Method) -> () {
        println!("Got Method: {:?}", mthd);
        ()
    }
    /// Provide target receiver
    fn impl_target(&mut self) -> impl RequestTargetReceiver {
        self
    }
    /// Provides request version
    fn req_version(&mut self, version: H11Version) -> () {
        println!("Got Version: {:?}", version);
        ()
    }
    /// Indicates start of headers processing and completion of request line
    fn req_start_headers(&mut self) -> RespIndicative {
        println!("Got req_start_headers");
        RespIndicative::GoAhead
    }
    /// Indicates start of payload processing and completion of headers
    fn req_start_payload(&mut self) -> RespIndicative {
        println!("Got req_start_payload");
        RespIndicative::GoAhead
    }
    /// Indicates completion of entire request
    fn req_complete(&mut self) -> RespIndicative {
        println!("Got req_complete");
        RespIndicative::GoAhead
    }
}

impl RequestTargetReceiver for &mut RickRoll {
    #[inline]
    fn req_target_init(&mut self, target_part: &[u8]) -> () {
        println!(
            "got req_target_init: {}",
            core::str::from_utf8(target_part).unwrap()
        );
        ()
    }
    #[inline]
    fn req_target_add(&mut self, target_part: &[u8]) -> () {
        println!(
            "got req_target_add: {}",
            core::str::from_utf8(target_part).unwrap()
        );
        ()
    }
    #[inline]
    fn req_target_finish(&mut self) -> () {
        println!("got req_target_finish");
        ()
    }
}

impl RequestHeaderReceiver for RickRoll {
    fn req_header<'h>(&mut self, hdr: H11Header<'h>) -> RespIndicative {
        println!("got req_header: {:?}", hdr);
        RespIndicative::GoAhead
    }
    fn req_headers_finish(&mut self) -> RespIndicative {
        println!("got req_headers_finish");
        RespIndicative::GoAhead
    }
}

impl RequestBodyReceiver for RickRoll {
    /// Receive a chunk of data
    fn payload_chunk(&mut self, _chunk: &[u8]) -> RespIndicative {
        todo!()
    }
}

impl ResponseSender for RickRoll {
    #[inline]
    fn resp_on_status(&mut self) -> RespComplete {
        println!("got resp_on_status");
        RespComplete::Resp2xx(Resp2xx::Ok)
    }
    #[inline]
    fn resp_on_headers<'h>(&'h mut self, at: usize) -> &'h [H11Header<'h>] {
        println!("got resp_on_headers at: {at}");
        if at == 0 {
            &self.headers
        } else {
            &[]
        }
    }
    #[inline]
    fn resp_body_len(&mut self) -> ResponseBodySize {
        println!("got resp_body_len");
        ResponseBodySize::Fixed(RICK_ROLL.len())
    }
    #[inline]
    fn resp_on_body(&mut self, len: usize) -> &[u8] {
        println!("got resp_on_body");
        let rick_len = RICK_ROLL.len();

        if self.payload_out_cursor.consumed == rick_len {
            println!("Rick-Roll is complete.");
            return &[];
        }

        let consumed = self.payload_out_cursor.consumed;
        let remaining_capacity = len;

        println!("Rick-Roll continue with consumed = {consumed} and remaining_capacity = {remaining_capacity}");

        if remaining_capacity > 0 {
            let roll_end = if rick_len < consumed + remaining_capacity {
                rick_len
            } else {
                consumed + remaining_capacity
            };
            self.payload_out_cursor.consumed += roll_end - consumed;
            println!("Rick-Roll continue {consumed} .. {roll_end}");
            println!(
                "Payload_out_cursor.consumed = {}",
                self.payload_out_cursor.consumed
            );
            &RICK_ROLL[consumed..roll_end]
        } else {
            println!("Rick-Roll has no remaining capacity out.");
            &[]
        }
    }
    fn resp_on_complete(&mut self) -> () {
        println!("got resp_on_complete");
        todo!()
    }
}

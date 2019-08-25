use std::thread;
use crossbeam_requests::channel;

fn main() {
    type RequestType = String;
    type ResponseType = String;
    let (requester, responder) = channel::<RequestType, ResponseType>();
    thread::spawn(move || {
        responder.poll_loop(|req, res_sender| res_sender.respond(req));
    });
    let msg = String::from("Hello");
    let receiver = requester.request(msg.clone()).unwrap();
    let res = receiver.collect().unwrap();
    assert_eq!(res, msg);
}

//! mpsc_requests rewritten for crossbeam, written by @stjepang (https://github.com/crossbeam-rs/crossbeam/issues/353#issuecomment-484013974)
//!
//! crossbeam_requests is a small library built on top of crossbeam-channel but with
//! the addition of the consumer responding with a message to the producer.
//! Since the producer no longer only produces and the consumer no longer only consumes, the
//! Producer is renamed to [RequestSender] and the Consumer is renamed to [RequestReceiver].
//!
//! This library is based on crossbeam-requests instead of mpsc channels in the standard library
//! because crossbeam has better performance and better compatibility with android.
//!
//! A perfect use-case for this library is single-threaded databases which need
//! to be accessed from multiple threads (such as SQLite)
//!
//! Here's a diagram of the dataflow
//!
//! |--------------------------------------------------------------------------------------|
//! | Thread    | Request thread |            Respond thread           |  Request thread   |
//! |--------------------------------------------------------------------------------------|
//! | Struct    | RequestSender ->  RequestReceiver  -> ResponseSender -> ResponseReceiver |
//! | (methods) |   (request)   -> (poll, poll_loop) ->    (respond)   ->    (collect)     |
//! |--------------------------------------------------------------------------------------|
//!
//! # Examples
//! For more examples, see the examples directory
//!
//! For even more examples see the tests in the tests directory
//!
//! ## Simple echo example
//! ```rust,run
//! use std::thread;
//! use crossbeam_requests::channel;
//!
//! type RequestType = String;
//! type ResponseType = String;
//! let (requester, responder) = channel::<RequestType, ResponseType>();
//! thread::spawn(move || {
//!     responder.poll_loop(|req, res_sender| {
//!         res_sender.respond(req);
//!     });
//! });
//! let msg = String::from("Hello");
//! let receiver = requester.request(msg.clone()).unwrap();
//! let res = receiver.collect().unwrap();
//! assert_eq!(res, msg);
//! ```

#![deny(missing_docs)]

use crossbeam_channel as cc;

/// Create a [RequestSender] and a [RequestReceiver] with a channel between them
///
/// The [RequestSender] can be cloned to be able to do requests to the same [RequestReceiver] from multiple
/// threads.
pub fn channel<Req, Res>() -> (RequestSender<Req, Res>, RequestReceiver<Req, Res>) {
    let (request_sender, request_receiver) = cc::unbounded::<(Req, ResponseSender<Res>)>();
    let request_sender = RequestSender::new(request_sender);
    let request_receiver = RequestReceiver::new(request_receiver);
    (request_sender, request_receiver)
}

#[derive(Debug)]
/// Errors which can occur when a [RequestReceiver] handles a request
pub enum RequestError {
    /// Error occuring when channel from [RequestSender] to [RequestReceiver] is broken
    RecvError,
    /// Error occuring when channel from [RequestReceiver] to [RequestSender] is broken
    SendError
}
impl From<cc::RecvError> for RequestError {
    fn from(_err: cc::RecvError) -> RequestError {
        RequestError::RecvError
    }
}
impl<T> From<cc::SendError<T>> for RequestError {
    fn from(_err: cc::SendError<T>) -> RequestError {
        RequestError::SendError
    }
}

/// A [ResponseSender] is received from the [RequestReceiver] to respond to the request back to the
/// [RequestSender]
pub struct ResponseSender<Res> {
    response_sender: cc::Sender<Res>,
}

impl<Res> ResponseSender<Res> {
    fn new(response_sender: cc::Sender<Res>) -> ResponseSender<Res> {
        ResponseSender {
            response_sender: response_sender,
        }
    }

    /// Responds a request from the [RequestSender] which finishes the request
    pub fn respond(&self, response: Res) {
        match self.response_sender.send(response) {
            Ok(_) => (),
            Err(_e) => panic!("Response failed, send pipe was broken during request!")
        }
    }
}

/// A [RequestReceiver] listens to requests. Requests are a tuple of a message
/// and a [ResponseSender] which is used to respond back to the [ResponseReceiver]
pub struct RequestReceiver<Req, Res> {
    request_receiver: cc::Receiver<(Req, ResponseSender<Res>)>,
}

impl<Req, Res> RequestReceiver<Req, Res> {
    fn new(request_receiver: cc::Receiver<(Req, ResponseSender<Res>)>) -> RequestReceiver<Req, Res> {
        RequestReceiver {
            request_receiver,
        }
    }

    /// Poll if the [RequestReceiver] has received any requests.
    /// The poll returns a tuple of the request message and a [ResponseSender]
    /// which is used to respond back to the ResponseReceiver.
    ///
    /// NOTE: It is considered an programmer error to not send a response with
    /// the [ResponseSender]
    ///
    /// This call is blocking
    /// TODO: add a poll equivalent which is not blocking and/or has a timeout
    pub fn poll(&self) -> Result<(Req, ResponseSender<Res>), RequestError> {
        match self.request_receiver.recv() {
            Ok((request, response_sender)) => Ok((request, response_sender)),
            Err(_e) => Err(RequestError::RecvError)
        }
    }

    /// A shorthand for running poll with a closure for as long as there is one or more [RequestSender]s alive
    /// referencing this [RequestReceiver]
    pub fn poll_loop<F>(&self, mut f: F) where F: FnMut(Req, ResponseSender<Res>) {
        loop {
            match self.poll() {
                Ok((request, response_sender)) => f(request, response_sender),
                Err(e) => match e {
                    // No more send channels open, quitting
                    RequestError::RecvError => break,
                    _ => panic!("This is a bug")
                }
            };
        }
    }
}


/// [ResponseReceiver] listens for a response from a [ResponseSender].
/// The response is received using the collect method.
#[derive(Clone)]
pub struct ResponseReceiver<Res> {
    response_receiver: cc::Receiver<Res>,
}

impl<Res> ResponseReceiver<Res> {
    fn new(response_receiver: cc::Receiver<Res>) -> ResponseReceiver<Res> {
        ResponseReceiver {
            response_receiver
        }
    }

    /// Send request to the connected [RequestReceiver]
    pub fn collect(&self) -> Result<Res, RequestError> {
        Ok(self.response_receiver.recv()?)
    }
}


/// [RequestSender] has a connection to a [RequestReceiver] to which it can
/// send a requests to.
/// The request method is used to make a request and it returns a
/// [ResponseReceiver] which is used to receive the response.
#[derive(Clone)]
pub struct RequestSender<Req, Res> {
    request_sender: cc::Sender<(Req, ResponseSender<Res>)>,
}

impl<Req, Res> RequestSender<Req, Res> {
    fn new(request_sender: cc::Sender<(Req, ResponseSender<Res>)>) -> RequestSender<Req, Res> {
        RequestSender {
            request_sender: request_sender,
        }
    }

    /// Send request to the connected [RequestReceiver]
    /// Returns a [RequestReceiver] which is used to receive the response.
    pub fn request(&self, request: Req) -> Result<ResponseReceiver<Res>, RequestError> {
        let (response_sender, response_receiver) = cc::unbounded::<Res>();
        let response_sender = ResponseSender::new(response_sender);
        self.request_sender.send((request, response_sender))?;
        Ok(ResponseReceiver::new(response_receiver))
    }
}

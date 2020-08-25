#[cfg(test)]
mod tests {
    extern crate mpsc_requests;

    use std::thread;

    use mpsc_requests::{channel, RequestSender};

    /// Check that the RequestSender and Receiver can be cloned even if the
    /// request and response types doesn't implement Clone
    #[test]
    fn test_clone() {
        struct NoClone;
        let (requester, responder) = channel::<NoClone, NoClone>();
        let req2 = requester.clone();
        let resp = req2.request(NoClone).unwrap();
        responder.poll().unwrap().1.respond(NoClone);
        let resp2 = resp.clone();
        resp2.collect().unwrap();
    }

    // Tests responding with same data as was sent
    #[test]
    fn test_echo() {
        let (requester, responder) = channel::<String, String>();
        thread::spawn(move || {
            responder.poll_loop(|req, res_sender| res_sender.respond(req));
        });
        let msg = String::from("test");
        let receiver = requester.request(msg.clone()).unwrap();
        let result = receiver.collect().unwrap();
        assert_eq!(result, msg);
    }

    // Tests different types of the Request and Reponse types
    #[test]
    fn test_wordcount() {
        let (requester, responder) = channel::<String, usize>();
        thread::spawn(move || {
            responder.poll_loop(|req, res_sender| res_sender.respond(req.len()));
        });
        let msg = String::from("test");
        let receiver = requester.request(msg.clone()).unwrap();
        let result = receiver.collect().unwrap();
        assert_eq!(result, 4);

        let msg = String::from("hello hello 123 123");
        let receiver = requester.request(msg.clone()).unwrap();
        let result = receiver.collect().unwrap();
        assert_eq!(result, 19);
    }

    // Example of returning Result from request
    #[test]
    fn test_result() {
        #[derive(Debug)]
        struct InvalidStringError;
        let (requester, responder) = channel::<String, Result<(), InvalidStringError>>();
        thread::spawn(move || {
            responder.poll_loop(|req, res_sender| {
                if req.starts_with("http://") {
                    res_sender.respond(Ok(()))
                } else {
                    res_sender.respond(Err(InvalidStringError))
                }
            });
        });
        let msg = String::from("http://test.com");
        let receiver = requester.request(msg).unwrap();
        let result = receiver.collect().unwrap();
        result.unwrap();

        let msg = String::from("invalid string");
        let receiver = requester.request(msg).unwrap();
        let result = receiver.collect().unwrap();
        assert!(result.is_err());
    }

    // Test multiple requesters on multiple threads with unique requests
    #[test]
    fn test_multiple_requesters() {
        use std::sync::atomic::AtomicUsize;
        use std::sync::atomic::Ordering;
        use std::sync::Arc;

        const N_THREADS : usize = 10;
        const N_REQUESTS_PER_THREAD : i64 = 1000;

        let (requester, responder) = channel::<String, String>();
        thread::spawn(move || {
            responder.poll_loop(|req, res_sender| res_sender.respond(req.clone()));
        });

        fn request_fn(requester: RequestSender<String, String>, ti: usize, atomic_counter: Arc<AtomicUsize>) {
            atomic_counter.fetch_add(1, Ordering::Acquire);
            thread::park();
            for i in 0..N_REQUESTS_PER_THREAD {
                let msg = format!("message from thread {} iteration {}", ti, i);
                let receiver = requester.request(msg.clone()).unwrap();
                let result = receiver.collect().unwrap();
                assert_eq!(result, msg);
            }
            atomic_counter.fetch_sub(1, Ordering::Acquire);
        }

        let mut threads = vec![];
        let atomic_counter = Arc::new(AtomicUsize::new(0));
        for i in 0..N_THREADS {
            let thread_p = requester.clone();
            let a = atomic_counter.clone();
            let t = thread::spawn(move || request_fn(thread_p, i, a));
            threads.push(t);
        }
        while atomic_counter.load(Ordering::Acquire) < N_THREADS {}
        for t in threads {
            t.thread().unpark();
        }
        while atomic_counter.load(Ordering::Acquire) > 0 {}
    }
}

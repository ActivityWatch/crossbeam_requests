use mpsc_requests::{RequestReceiver, RequestSender, channel};
use std::sync::Arc;
use std::sync::Mutex;

type RequestType = String;
type ResponseType = String;
type Handler = fn(request: RequestType) -> ResponseType;

struct WorkerPool {
    pub num_threads: usize,
    pub worker_index: usize,
    pub thread_locks_arc: Arc<Vec<Mutex<bool>>>,
    pub requesters_arc: Arc<Vec<RequestSender<RequestType, ResponseType>>>,
    pub responders_arc: Arc<Vec<RequestReceiver<RequestType, ResponseType>>>
}

impl WorkerPool {
    pub fn new(num_threads: usize, handler: Handler) -> WorkerPool {
        let mut thread_locks = vec![];
        let mut requesters = vec![];
        let mut responders = vec![];
        // build channels
        for _ in 0..num_threads {
            let (requester, responder) = channel::<RequestType, ResponseType>();
            thread_locks.push(Mutex::new(false));
            requesters.push(requester);
            responders.push(responder);
        }
        // spawn handler threads
        let thread_locks_arc = Arc::new(thread_locks);
        let requesters_arc = Arc::new(requesters);
        let responders_arc = Arc::new(responders);
        for i in 0..num_threads {
            let responders_arc = responders_arc.clone();
            let thread_locks_arc = thread_locks_arc.clone();
            std::thread::spawn(move || {
                let responder = responders_arc.get(i).unwrap();
                responder.poll_loop(|req, response_sender| {
                    let _ = thread_locks_arc[i].lock().unwrap();
                    let response = handler(req);
                    response_sender.respond(response);
                });
            });
        }
        return WorkerPool {
            num_threads,
            worker_index: 0,
            thread_locks_arc,
            requesters_arc,
            responders_arc
        };
    }

    pub fn work(&mut self, request: RequestType) -> ResponseType {
        let requester = self.requesters_arc.get(self.worker_index).unwrap();
        let receiver = requester.request(request.clone()).unwrap();
        let response = receiver.collect().unwrap();
        self.worker_index += 1;
        if self.worker_index == self.num_threads {
            self.worker_index = 0;
        }
        return response;
    }
}

fn main() {
    let num_threads = 4;
    let handler = |request: RequestType| {
        println!("request = {}", request);
        return String::from("response");
    };
    let mut worker_pool = WorkerPool::new(num_threads, handler);
    for _ in 0..100 {
        let request = String::from("request");
        let response = worker_pool.work(request);
        println!("response = {}", response);
    }
}

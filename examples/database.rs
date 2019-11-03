use std::thread;
use std::collections::HashMap;
use mpsc_requests::channel;
#[derive(Debug)]
enum Errors {
    NoSuchPerson,
}
enum Commands {
    CreateUser(String, u64),
    GetUser(String)
}
#[derive(Debug)]
enum Responses {
    Success(),
    GotUser(u64)
}

fn main() {
    let (requester, responder) = channel::<Commands, Result<Responses, Errors>>();
    thread::spawn(move || {
        let mut age_table : HashMap<String, u64> = HashMap::new();
        responder.poll_loop(|request, res_sender| {
            res_sender.respond(match request {
                Commands::CreateUser(user, age) => {
                    age_table.insert(user, age);
                    Ok(Responses::Success())
                },
                Commands::GetUser(user) => {
                    match age_table.get(&user) {
                        Some(age) => Ok(Responses::GotUser(age.clone())),
                        None => Err(Errors::NoSuchPerson)
                    }
                }
            });
        });
    });

    // Create user
    let username = String::from("George");
    let cmd = Commands::CreateUser(username.clone(), 64);
    let receiver = requester.request(cmd).unwrap();
    receiver.collect().unwrap().unwrap();
    // Fetch user and verify data
    let command = Commands::GetUser(username.clone());
    let receiver = requester.request(command).unwrap();
    match receiver.collect().unwrap().unwrap() {
        Responses::GotUser(age) => assert_eq!(age, 64),
        _ => panic!("Wrong age")
    }
    // Try to fetch non-existing user
    let username = String::from("David");
    let command = Commands::GetUser(username);
    let receiver = requester.request(command).unwrap();
    let result = receiver.collect().unwrap();
    assert!(result.is_err());
}

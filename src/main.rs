use actix_cors::Cors;
use actix_web::{
    web::{self, Data},
    App, HttpRequest, HttpResponse, HttpServer,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::{
    fs::{create_dir, OpenOptions},
    io::{BufRead, BufReader, BufWriter, ErrorKind, Read, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex},
};

#[derive(Debug, Serialize, Deserialize)]
struct SendCodeRequest {
    username: String,
    code: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SendCodeResponse {
    stdout: Vec<String>,
    stderr: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateSessionRequest {
    username: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorResponse {
    code: i32,
    error: String,
}

impl ErrorResponse {
    pub fn new(code: i32, error: String) -> ErrorResponse {
        return ErrorResponse {
            code: code,
            error: error,
        };
    }
}

#[derive(Debug)]
struct StaticData {
    processes: HashMap<String, (BufReader<ChildStdout>, BufWriter<ChildStdin>)>,
}

fn write_whole_file(filepath: String, content: &Vec<String>) -> Result<(), String> {
    let file_result = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(filepath);
    let mut file = match file_result {
        Ok(file) => file,
        Err(_) => return Err("Could not open file with users code".to_string()),
    };

    /*
     * Clear all the contents of the file with users code
     */
    let truncate_result = file.set_len(0);
    match truncate_result {
        Ok(_) => (),
        Err(_) => return Err("Could not clear contents of users file".to_string()),
    };

    /*
     * Write code sent by a user into the file in his directory.
     * Code is sent as an array of strings, so we must join it
     * with newline character between before writing to the file.
     */
    let write_result = file.write_all(content.join("\n").as_bytes());
    match write_result {
        Ok(_) => (),
        Err(_) => return Err("Could not write users code into users file".to_string()),
    };
    Ok(())
}

fn read_until_wait(reader: &mut BufReader<ChildStdout>) -> Result<String, String> {
    let mut output: String = String::new();
    loop {
        let mut line: String = String::new();
        match reader.read_line(&mut line) {
            Ok(_) => (),
            Err(_) => return Err("Could not read line from stdout".to_string()),
        }
        output += &line;
        if line.contains("{ \"base_class_name\" : \"Action\", \"class_name\" : \"WaitForCode\", \"properties\" : {} }") {
            break;
        }
    }
    Ok(output)
}

/*
 * Creates container process and returns ownership of created:
 *  - Writer
 *  - Reader
 */
fn create_container(
    username: String,
    interactive: bool,
) -> Result<(BufReader<ChildStdout>, BufWriter<ChildStdin>), String> {
    /*
     * Create directory for users code
     * TODO: check if it already exists
     */
    let dir = "./usr/".to_string() + &username;
    let create_dir_result = create_dir(dir);
    match create_dir_result {
        Ok(_) => (),
        Err(error) => match error.kind() {
            ErrorKind::AlreadyExists => (),
            _ => {
                return Err("Could not create users directory".to_string());
            }
        },
    }

    /*
     * Create path to users directory where his code will be stored
     * and create argument string for a volume that is passed to
     * podman-run. It has to be ":ro" (i.e read-only), so the user
     * is not able to create any files in directory shared between
     * host and users container.
     */
    let dir = "./usr/".to_string() + &username;
    let volume = format!("{}{}", dir, ":/code:ro");

    let interactive_arg: &str;
    if interactive {
        interactive_arg = "--interactive";
    } else {
        interactive_arg = "";
    }
    /*
     * Execute users code in a safe podman container, the containers are set to timeout
     * after 2 seconds. I wanted to limit their memory, but I had issues with doing that
     * in a rootless container. Code sent by the user can be found in directory /usr/username
     * and is binded to the container using "-v" option, so the container can read it's contents
     * and execute it.
     * TODO: Limit containters memory
     */
    let process_result = Command::new("podman")
        .arg("run")
        //.arg("-m")
        //.arg("256m")
        .arg(interactive_arg) // Interactive option of `podman`
        .arg("--timeout")
        .arg("3600")
        .arg("-v")
        .arg(volume)
        .arg("lynx-runtime:0.4")
        .arg(interactive_arg) // Interactive option of `runtime`
        .arg("/code/code.py")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn();

    let process = match process_result {
        Ok(process) => process,
        Err(_) => return Err("Could not run users container".to_string()),
    };

    let stdout = match process.stdout {
        Some(stdout) => stdout,
        None => return Err("Could not access child process stdout".to_string()),
    };

    let stdout_reader = BufReader::new(stdout);

    let stdin = match process.stdin {
        Some(stdin) => stdin,
        None => return Err("Could not access child process stdin".to_string()),
    };

    let stdin_writer = BufWriter::new(stdin);

    Ok((stdout_reader, stdin_writer))
}

/*
 *
 */
fn run_code(
    username: String,
    code: &Vec<String>,
    data: Data<Mutex<StaticData>>,
) -> Result<(String, String), String> {
    /*
     * Write users code do code.py
     */
    let code_path = "./usr/".to_string() + &username + "/code.py"; /* It's not done properly I think */
    let write_code_result = write_whole_file(code_path, code);
    match write_code_result {
        Ok(_) => (),
        Err(value) => return Err(value),
    };

    /*
     * Get process to run the code in
     */
    let mut my_data = data.lock().unwrap();
    let process_option = my_data.processes.get_mut(&username);

    let process_tuple = match process_option {
        Some(process_tuple) => process_tuple,
        None => return Err("There is no interactive session for the user!".to_string()),
    };

    let reader = &mut process_tuple.0;
    let writer = &mut process_tuple.1;

    /*
     * We have access to processes buffers by (creating new container or) using the interactive one,
     * Now we should signal that the runtime can continue execution
     */
    match writer.write_all(b"CODE UPLOADED\n") {
        Ok(_) => (),
        Err(e) => println!("{}", e.to_string()),
    };
    match writer.flush() {
        Ok(_) => (),
        Err(e) => println!("{}", e.to_string()),
    };

    let output = match read_until_wait(reader) {
        Ok(value) => value,
        Err(value) => return Err(value),
    };

    println!("STDOUT:\n{output}");
    println!("STDERR:\n");

    Ok((output.clone(), output.clone()))
}

async fn send_code(
    item: web::Json<SendCodeRequest>,
    data: Data<Mutex<StaticData>>,
    _: HttpRequest,
) -> HttpResponse {
    /*
     * TODO: Add session handling in order to get
     * proper usernames
     */
    let (stdout, stderr) = match run_code(item.username.clone(), &item.code, data) {
        Ok(value) => value,
        Err(err) => return HttpResponse::Ok().json(ErrorResponse::new(-1, err)),
    };

    /*
     * We return the code as a list of strings, so we must split it
     * by the newline character.
     */
    let output = SendCodeResponse {
        stdout: stdout.split("\n").map(str::to_string).collect(),
        stderr: vec!["".to_string()],
    };

    HttpResponse::Ok().json(output) // <- send json response
}

async fn create_session(
    request: web::Json<CreateSessionRequest>,
    data: Data<Mutex<StaticData>>,
    _: HttpRequest,
) -> HttpResponse {
    let mut my_data = data.lock().unwrap();

    // if my_data.processes.contains_key(&request.username) {
    //     return HttpResponse::Ok().json(ErrorResponse::new(-1, "Session already exists".to_string()));
    // }

    let container_result = create_container(request.username.clone(), true);
    let (mut reader, writer) = match container_result {
        Ok(container) => container,
        Err(_) => {
            return HttpResponse::Ok().json(ErrorResponse::new(
                -1,
                "Could not create new container".to_string(),
            ))
        }
    };

    let output = match read_until_wait(&mut reader) {
        Ok(output) => output,
        Err(_) => {
            return HttpResponse::Ok().json(ErrorResponse::new(
                -1,
                "Could not read new containers output".to_string(),
            ))
        }
    };

    my_data
        .processes
        .insert(request.username.clone(), (reader, writer));

    /*
     * We use the same kind of response as in `send_code`, so
     * the frontends can easily show newly created scene
     */
    let response = SendCodeResponse {
        stdout: output.split("\n").map(str::to_string).collect(),
        stderr: vec!["".to_string()],
    };

    HttpResponse::Ok().json(response)
}

fn help() {
    println!(
        "usage:
lynx-runner <PORT>
    Run lynx-runner at PORT"
    );
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let port: u16;

    match args.len() {
        2 => {
            port = match args[1].parse::<u16>() {
                Ok(port) => port,
                Err(_) => {
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidInput,
                        "Could not parse port",
                    ))
                }
            };
        }
        1 => {
            port = 9000;
        }
        // all the other cases
        _ => {
            // show a help message
            help();
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "Not enough arguments",
            ));
        }
    }

    let data = Data::new(Mutex::new(StaticData {
        processes: HashMap::new(),
    }));

    /*
     * Create runner server
     */
    HttpServer::new(move || {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .service(web::resource("/send_code").route(web::post().to(send_code)))
            .service(web::resource("/create_session").route(web::post().to(create_session)))
            .app_data(Data::clone(&data))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}

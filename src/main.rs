use artnet_protocol::*;
use std::net::{UdpSocket, ToSocketAddrs};
use std::time::Instant;

fn main() {

    // Open an udp port to listen to artnet
    let socket = UdpSocket::bind("0.0.0.0:6454").expect("Could not bind to port 6454");
    socket.set_broadcast(true).expect("Could not set broadcast");
    socket.set_nonblocking(true).expect("Could not set nonblocking");

    // define the brodcast adress for polling
    let brodcast = "255.255.255.255:6454".to_socket_addrs().expect("Test").next().expect("Test");


    #[derive(Debug)]
    struct Node {
        ip: Ipv4Addr,
        port: u16,
        last_reply : Instant
    }
    let mut nodes: Vec<Node> = Vec::new();

    #[derive(Debug)]
    struct PortAddressSubscribers {
        port_address: PortAddress,
        subscribers: Vec<Node>
    }

    let mut subscriptions: Vec<PortAddressSubscribers> = Vec::new();

    // start the main loop
    let mut  start = Instant::now();
    loop {

        // send a poll packet every 2 seconds
        let elapsed = start.elapsed();
        if elapsed.as_secs() >= 4 {
            start = Instant::now();

            // Send a Poll Packet
            let buff = ArtCommand::Poll(Poll::default()).write_to_buffer().expect("Polling failed");
            socket.send_to(&buff, &brodcast).expect("Polling failed");

            println!("Sent Poll Packet");

            // Print node list
            println!("Nodes: {:#?}", nodes);
        }

        // remove nodes that have not responded
        nodes.retain(|node| node.last_reply.elapsed().as_secs() < 20);

        // Create a Buffer for storing the current Packet
        let mut buffer = [0u8; 1024];   

        match socket.recv_from(&mut buffer) {
            Ok((len, src)) => {
                // Parse and handle the packet as before
                let command = ArtCommand::from_buffer(&buffer[..len]).expect("Malformed Packet");
                // Handle the command types
                match command {

                    // If we have a DMX Packet
                    ArtCommand::Output(output) => {
                        println!("Received DMX Packet with {:?} bytes", output.port_address);


                        let address = &output.port_address;

                        if *address > PortAddress::from(0) {
                            let output_bytes = output.to_bytes().expect("Parsing failed");

                            for node in &mut nodes {
                                for port in &node.port_address {
                                    if port == address {
                                        let command = ArtCommand::Output(Output::from(&output_bytes).expect("Parsing failed"));
                                        let bytes = command.write_to_buffer().expect("Parsing failed");
                                        let src_addr = ( node.ip.clone() + ":6454" ).to_socket_addrs().expect("Test").next().expect("Test");
                                        socket.send_to(&bytes, &src_addr).expect("Sending failed");
                                    }
                                }


                                /*if &node.port_address == address {
                                    //println!("Sending DMX Packet to {}", node.ip);
                                    let command = ArtCommand::Output(Output::from(&output_bytes).expect("Parsing failed"));
                                    let bytes = command.write_to_buffer().expect("Parsing failed");
                                    let src_addr = ( node.ip.clone() + ":6454" ).to_socket_addrs().expect("Test").next().expect("Test");
                                    socket.send_to(&bytes, &src_addr).expect("Sending failed");
                                }*/
                            }
                        }
                    },

                    // Wen revieving a Poll Packet
                    ArtCommand::Poll(poll) => {
                        //println!("Recieved Poll Packet from {}", src);

                        // define a reply for polling
                        let poll_reply = ArtCommand::PollReply (Box::new(PollReply {

                            ..PollReply::default()
                        }));
               
                        // send the reply
                        let bytes = poll_reply.write_to_buffer().expect("Parsing reply failed");
                        socket.send_to(&bytes, &src).expect("Sending reply failed");
                    },

                    ArtCommand::PollReply(poll_reply) => {
                        let port_address = &poll_reply.port_address;



                    },
                    // default
                    _ => println!("Received Packet of type: {:?} from {}", command, src)
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No packet available - continue loop immediately
            }
            Err(e) => panic!("Receive error: {}", e),
        }
    }
}

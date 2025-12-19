use artnet_protocol::*;
use std::net::{UdpSocket, ToSocketAddrs, SocketAddr};
use std::time::{ Instant, Duration };

use std::collections::HashMap;


fn main() {

    // Open an udp port to listen to artnet
    let socket = UdpSocket::bind("0.0.0.0:6454").expect("Could not bind to port 6454");
    socket.set_broadcast(true).expect("Could not set broadcast");
    socket.set_nonblocking(true).expect("Could not set nonblocking");

    // define the brodcast adress for polling
    let brodcast = "255.255.255.255:6454".to_socket_addrs().expect("Test").next().expect("Test");

    // Nodes Per port address
    let mut subscriptions: HashMap<PortAddress, HashMap<SocketAddr, Instant>> = HashMap::new();

    // start the main loop
    let mut start = Instant::now();
    loop {

        // send a poll packet every 3 seconds
        let elapsed = start.elapsed();
        if elapsed.as_secs() >= 3 {
            start = Instant::now();

            // Send a Poll Packet
            let buff = ArtCommand::Poll(Poll::default()).write_to_buffer().expect("Polling failed");
            socket.send_to(&buff, &brodcast).expect("Polling failed");

            // Clean unresponsive Nodes
            let now = Instant::now();

            // Iterate through all subscriptions
            subscriptions.retain(|_, addr_map| {
                // Retain adresses of nodes that were alive the last 20s
                addr_map.retain(|_, instant| now.duration_since(*instant) <= Duration::from_secs(20));
                !addr_map.is_empty()
            });

            //println!("Subscriptions: {:?}", subscriptions.entry(PortAddress::from(1)));
        }

        // Create a Buffer for storing the current Packet
        let mut buffer = [0u8; 1024];

        // Check if a packet was recieved
        match socket.recv_from(&mut buffer) {

            // Packet is avalliable
            Ok((len, src)) => {

                // Parse the Packet
                // ToDo: no panic if parsing fails
                let command = ArtCommand::from_buffer(&buffer[..len]).expect("Malformed Packet");

                // Handle the command types
                match command {

                    // If we have a DMX Packet
                    ArtCommand::Output(output) => {
                        //println!("Received DMX Packet with {:?} bytes", output.port_address);


                        let address = &output.port_address;

                        // Do not relay Packets from PortAddress 0
                        // -> unconfigured devices automatically subscribe to it. So it generates network congestion.
                        if *address > PortAddress::from(0) {
                            let output_bytes = output.to_bytes().expect("Parsing failed");

                            // iterate nodes in PortAddress
                            for (socket_addr, _instant) in subscriptions.get(&address).unwrap_or(&HashMap::new()) {

                                let command = ArtCommand::Output(Output::from(&output_bytes).expect("Parsing failed"));
                                let bytes = command.write_to_buffer().expect("Parsing failed");


                                socket.send_to(&bytes, &socket_addr).expect("sending Failed!");

                                //println!("SentDMX!!");
                            }
                        }
                    },

                    // Wen revieving a Poll Packet
                    ArtCommand::Poll(_poll) => {

                        // define a reply for polling
                        let poll_reply = ArtCommand::PollReply (Box::new(PollReply {

                            ..PollReply::default()
                        }));
               
                        // send the reply
                        let bytes = poll_reply.write_to_buffer().expect("Parsing reply failed");
                        socket.send_to(&bytes, &src).expect("Sending reply failed");
                    },

                    ArtCommand::PollReply(poll_reply) => {

                        // Shift the NetSwitch and SubSwich fields to the right position of PortAddress
                        let mut port_net: u16 = u16::from(poll_reply.port_address[0]) << 8;
                        port_net = port_net | u16::from(poll_reply.port_address[1]) << 4;

                        let instant = Instant::now();

                        // Check the Swin ports
                        for i in 0..4 {
                            // Complete the PortAddress
                            let port_address: PortAddress = (port_net | &poll_reply.swin[i].into()).try_into().unwrap();

                            // Get or create a new HashMap for the given PortAddress
                            let addr_map = subscriptions.entry(port_address).or_insert_with(HashMap::new);
                            addr_map.insert(src, instant);

                            //println!("found Port Address: {:?}", port_address);
                        }

                        // Check the Swout ports
                        for i in 0..4 {
                            // Complete the PortAddress
                            let port_address: PortAddress = (port_net | &poll_reply.swout[i].into()).try_into().unwrap();

                            // Get or create a new HashMap for the given PortAddress
                            let addr_map = subscriptions.entry(port_address).or_insert_with(HashMap::new);
                            addr_map.insert(src, instant);

                            //println!("found Port Address: {:?}", port_address);
                        }

                    },
                    // On other packet types
                    _ => println!("Received Packet of type: {:?} from {}", command, src)
                }
            }

            // No Packets available, continue
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            }
            Err(e) => panic!("Receive error: {}", e),
        }
    }
}

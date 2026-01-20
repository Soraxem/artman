/// This is the ArtMan ArtNet manager
/// 
/// This software is for managing Artnet networks and Protocol upgrading to the Artnet 4 Spec of communication.

use artnet_parser::ArtPollFlags;
use artnet_parser::art_poll::ArtPoll;
use artnet_parser::art_poll_reply::ArtPollReply;
use artnet_parser::art_dmx::ArtDmx;
use artnet_parser::ArtNetPacket;
use artnet_parser::PortAddress;
use artnet_parser::get_op_code;
use artnet_parser::is_artnet;

use std::net::{ UdpSocket, ToSocketAddrs, SocketAddr };
use std::time::{ Instant, Duration };

use std::collections::HashMap;


// function to pad fixed binary strings
const fn to_fixed<const N: usize>(input: &[u8]) -> [u8; N] {
    let mut buffer = [0u8; N];
    let mut i = 0;
    while i < input.len() && i < N {
        buffer[i] = input[i];
        i += 1;
    }
    buffer
}




fn main() {

    // Get the Local IP
    let local_ip = {
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Couldn't bind dummy");
        socket.connect("8.8.8.8:80").expect("Couldn't connect to dummy");
        socket.local_addr().expect("Couldn't get local addr").ip()
    };
    println!("Detected Local IP: {}", local_ip);

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
            let buff = ArtPoll::default().serialize();
            socket.send_to(&buff, &brodcast).expect("Polling failed");

            // Clean unresponsive Nodes
            let now = Instant::now();

            // Iterate through all subscriptions
            subscriptions.retain(|_, addr_map| {
                // only retain adresses of nodes that were alive the last 20s
                addr_map.retain(|_, instant| now.duration_since(*instant) <= Duration::from_secs(20));
                !addr_map.is_empty()
            });

            // Show a list of subscriptions to universe 1
            println!("Subscriptions: {:?}", subscriptions.entry( PortAddress::unsafe_from_u16(1) ));
        }

        // Create a Buffer for storing the current Packet
        let mut buffer = [0u8; 1024];

        // Check if a packet was recieved
        match socket.recv_from(&mut buffer) {

            // Packet is avalliable
            Ok((len, src)) => {

                // Parse the recieved udp Packet
                match ArtNetPacket::parse(&buffer[..len]) {
                    // The packet was parsed
                    Ok(packet) => {
                        // match the packet format
                        match packet {

                            // Recieved a polling request
                            ArtNetPacket::ArtPoll(poll) => {

                                // setup a reply
                                let reply = ArtPollReply {
                                    ip_address: match local_ip {
                                        std::net::IpAddr::V4(ipv4) => ipv4,
                                        std::net::IpAddr::V6(_) => panic!("IPv6 not supported"),
                                    },

                                    port_name: to_fixed(b"ArtMan"),
                                    long_name: to_fixed(b"development version of Artman"),
                                    version_info: 14,
                                    ..ArtPollReply::default()
                                };

                                // Send the Reply
                                let bytes = reply.serialize();
                                socket.send_to(&bytes, &src).expect("Sending reply failed");

                            },

                            // Recieved a polling replay, store it to the subscription list
                            ArtNetPacket::ArtPollReply(poll_reply) => {
                                
                                // Take the Current time to remember when the node was last seen
                                let instant = Instant::now();

                                // iterate over all inputs and outputs, and resgistering subscriptions for them
                                for port_address in poll_reply.inputs.iter() {
                                    // Get or create a new HashMap for the given PortAddress
                                    let addr_map = subscriptions.entry(*port_address).or_insert_with(HashMap::new);
                                    addr_map.insert(src, instant);
                                }
                                for port_address in poll_reply.outputs.iter() {
                                    // Get or create a new HashMap for the given PortAddress
                                    let addr_map = subscriptions.entry(*port_address).or_insert_with(HashMap::new);
                                    addr_map.insert(src, instant);
                                }

                            },

                            // Recieved a DMX Packet
                            ArtNetPacket::ArtDmx(dmx) => {
                                //println!("ArtDmx for PortAddress: {} first Channel: {} sequence: {}", dmx.port_address.0, dmx.data[0], dmx.sequence);

                                // Only forward dmx that is not universe 0
                                // Universe 0 is the default setting, and therefore congests the network, if every unconfigured device unkowingly subscribes to universe 0
                                if dmx.port_address.as_u16() > 0 {

                                    // iterate nodes that subscribed to this universe
                                    for (socket_addr, _instant) in subscriptions.get(&dmx.port_address).unwrap_or(&HashMap::new()) {

                                        // forward the dmx data
                                        socket.send_to(&dmx.serialize(), &socket_addr).expect("sending Failed!");
                                    }
                                }
                            },
                            _ => println!("Unknown Packet Type"),
                        }
                    },

                    // The packet could not be parsed
                    Err(error) => {
                        println!("Error: {}", error);
                    }
                }

            }

            // No Packets available, continue
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            }
            Err(e) => panic!("Receive error: {}", e),
        }
    }
}

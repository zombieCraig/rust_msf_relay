Rust Metasploit HW Relay
------------------------

This is an implementation of the [Metasploit Hardware bridge](https://blog.rapid7.com/2017/02/02/exiting-the-matrix/)
written in Rust.  This project has two purposes:

# Teach myself rust
# Create a binary HW bridge that can be deployed remotely

The first point will be obvious to any Rustaceans looking at the
code.  It's really hard not to constantly refactor when learning rust,
mainly because you learn new and better ways to do things as you go.
I forced myself to just make it work first and then refactor to something
easier to maintain and expand on.  If you love rust and want to help,
please do!  Ultimately it would be good to have the HW Bridge relay be
a module and you include modules for automotive, rf, etc.

The second point is a bit more technically interesting.  This orginally
started because I needed a relay for a custom "cape" for a Beagle Bone Black
and instead of writing it in C, rust was the better option.  It has the
additional benefit of being deploy-able after an exploit.  This allows you
to extend you kill chain and "pivot" into raw hardware via metasploit.
Currently, this isn't built into Metasploit but that will be the next thing
I will look into doing.  Example usage:  Pop access on an Infotainment unit,
push a binary version of the Relay to the IVI and connect to interact with
the raw hardware...all within Metasploit.

Note:  This is just the relay and not the full MSF implementation...yet.

Compiling
---------
Obviously you will need rust and this version is based on SocketCAN so you will also need Linux.

We use Rocket for the web services.  Rocket comes with three build environments: dev, stage and prod.
See [Rocket Configuration](https://rocket.rs/guide/configuration/) for details.

* cargo build (Builds a development version)
* or alternatively: ROCKET_ENV=prod cargo run (to launch in production)

Building puts the binary under the target folder while the "run" option will compile and launch.

Usage
-----
The usage is fairly straight forward at the moment.  Currently the only thing you need to be aware
of is that you need some CAN interfaces already setup (ie: vcan, can, slcan).  These must be
provided on the command line.  You can list as many can interfaces as you would like.

* target/debug/msf_relay vcan0 can0

Once running you can connect to the device via Metasploit with:

* msf> use auxiliary/client/hwbridge/connect

Setup the parameters to match your server then run.  Once connected you will have a session for
the hardware bridge.  Type 'sessions' to see.  Then you can interact with the bridge directly:

* msf> sessions -i 1  # Assuming your hwbridge session is #1
* hwbridge> status

Known Issues
------------
* Command line web options (like port) do not work
* Authentication isn't builtin
* Https is not builtin

use std::io;
use std::net::SocketAddr;

use mio::{TryRead, TryWrite};
use mio::tcp::TcpStream;
use openssl::ssl::NonblockingSslStream;
use openssl::ssl::error::NonblockingSslError;

use result::{Result, Error, Kind};

use self::Stream::*;
pub enum Stream {
    Tcp(TcpStream),
    Tls {
        sock: NonblockingSslStream<TcpStream>,
        negotiating: bool,
    }
}


impl Stream {

    pub fn tcp(stream: TcpStream) -> Stream {
        Tcp(stream)
    }

    pub fn tls(stream: NonblockingSslStream<TcpStream>) -> Stream {
        Tls { sock: stream, negotiating: false }
    }

    pub fn is_tls(&self) -> bool {
        match *self {
            Tcp(_) => false,
            Tls {..} => true,
        }
    }

    pub fn evented(&self) -> &TcpStream {
        match *self {
            Tcp(ref sock) => sock,
            Tls { ref sock, ..} => sock.get_ref(),
        }
    }

    pub fn is_negotiating(&self) -> bool {
        match *self {
            Tcp(_) => false,
            Tls { sock: _, ref negotiating } => *negotiating,
        }

    }

    pub fn clear_negotiating(&mut self) -> Result<()> {
        debug!("Clearing negotiating status for {}", try!(self.peer_addr()));
        match *self {
            Tcp(_) => Err(Error::new(Kind::Internal, "Attempted to clear negotiating flag on non ssl connection.")),
            Tls { sock: _, ref mut negotiating } => Ok(*negotiating = false),
        }
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        match *self {
            Tcp(ref sock) => sock.peer_addr(),
            Tls { ref sock, ..} => sock.get_ref().peer_addr(),
        }
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        match *self {
            Tcp(ref sock) => sock.local_addr(),
            Tls { ref sock, ..} => sock.get_ref().local_addr(),
        }
    }
}

impl TryRead for Stream {

    fn try_read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        match *self {
            Tcp(ref mut sock) => sock.try_read(buf),
            Tls { ref mut sock, ref mut negotiating } => {
                match sock.read(buf) {
                    Ok(cnt) => Ok(Some(cnt)),
                    Err(NonblockingSslError::SslError(err)) =>
                        Err(io::Error::new(io::ErrorKind::Other, err)),
                    Err(NonblockingSslError::WantWrite) => {
                        *negotiating = true;
                        Ok(None)
                    }
                    Err(NonblockingSslError::WantRead) => Ok(None),
                }
            }
        }
    }
}

impl TryWrite for Stream {

    fn try_write(&mut self, buf: &[u8]) -> io::Result<Option<usize>> {
        match *self {
            Tcp(ref mut sock) => sock.try_write(buf),
            Tls { ref mut sock, ref mut negotiating } => {

                *negotiating = false;

                match sock.write(buf) {
                    Ok(cnt) => Ok(Some(cnt)),
                    Err(NonblockingSslError::SslError(err)) =>
                        Err(io::Error::new(io::ErrorKind::Other, err)),
                    Err(NonblockingSslError::WantRead) => {
                        *negotiating = true;
                        Ok(None)
                    }
                    Err(NonblockingSslError::WantWrite) => Ok(None),
                }
            }
        }
    }
}

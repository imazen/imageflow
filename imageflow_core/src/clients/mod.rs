pub mod stateless;
pub mod fluent;


//
//
//pub trait BasicClient{
//    //Does the server support URL fetches? Local filenames? If so, within which domains/directories?
//    //Client-local filenames work for tool and lib, but not server.
//    //URLs should work everywhere, but depending on security policy
//    //Servers may make local files acessible via some kind of identifier - perhaps a virtual path?
//
//    //Does the server support any kind of session/context/state? If not, we have to adapt.
//
//}
//
//pub struct LibClient{
//
//
//
//}
//
//impl LibClient {
//    pub fn create_session() -> LibClientSession{
//
//    }
//}
//
////Bytes in, bytes out (starting point!)
//
////Later we can add these things (which involve more security and caching aspects)
////read client file
////write client file - Maybe skip this one??
////access server resource
////read URL
//
//
//pub enum ClientIo{
//    ReadFile(String),
//    WriteFile(String),
//    Url(String),
//
//
//}
//
//pub struct ClientBasicBuild{
//    //Framewise
//    //DecoderHints
//    //BuildConfig
//}
//
//pub struct ClientBasicResult{
//    //list of io_id and Vec<8>
//    //Or failure detail?
//    //timings/resource info
//    //
//}
//
////Sessions can only have as much state as they can universally fake
//pub struct LibClientSession{
//
//}
//impl LibClientSession{
//
//    pub fn add_input(&mut self, io_id: i32, bytes: Vec<u8>) -> Result(){
//
//    }
//    pub fn get_image_info(&mut self, io_id: i32) -> Result<s::ImageInfo>{
//
//    }
//    pub fn build_basic(&mut self, Frame)
//
//    //Do work, get results (or error)
//
//    //Results include:
//    //Get output I/O bytes (owned)
//    //Get output dimensions, content type, preferred extension etc.
//
//}
//! Document-start and resume scripts for the desktop host.
//! This module does not link the window shell.

/// WKWebView ignores `-webkit-app-region`. These rectangles are what the
/// macOS `sendEvent:` hook hit-tests. The script waits for the native
/// `a3sDrag` handler, which is present on the main renderer even when the
/// document-start user script did not run again. Double-clicks are forwarded
/// so drag.js can zoom on mouse-up. A control only blocks the top band when
/// it is the element under the pointer. A focusable scroller is not a
/// control: its box covers the empty title band. Message bodies still block
/// so text in that band stays selectable. Controls covered by the workbench
/// home must not keep that empty titlebar from dragging.
const MACOS_WINDOW_DRAG_SCRIPT: &str = r#"(function(){
if(window.__A3S_WINDOW_DRAG__||window.__A3S_WINDOW_DRAG_BOOT__)return;
window.__A3S_WINDOW_DRAG_BOOT__=true;
var ATTR='data-tauri-drag-region';
var CLICKABLE=new Set(['A','BUTTON','INPUT','SELECT','TEXTAREA','LABEL','SUMMARY']);
var ROLES=new Set(['button','link','menuitem','tab','checkbox','radio','switch','option']);
var tries=0;
function clickable(el){
return CLICKABLE.has(el.tagName)||(el.hasAttribute('contenteditable')&&el.getAttribute('contenteditable')!=='false')||ROLES.has(el.getAttribute('role'));
}
function messageBody(el){
var ui=el.getAttribute('data-ui');
return !!ui&&(' '+ui+' ').indexOf(' part:message-content ')!==-1;
}
function shown(el){
var style=getComputedStyle(el);
return style.display!=='none'&&style.visibility!=='hidden'&&style.pointerEvents!=='none';
}
function boxes(el,into){
var rects=el.getClientRects();
for(var i=0;i<rects.length;i++){
var r=rects[i];
if(r.width>=1&&r.height>=1)into.push([r.x,r.y,r.width,r.height]);
}
}
function overlaps(box,regions){
for(var i=0;i<regions.length;i++){
var region=regions[i];
if(box[0]<region[0]+region[2]&&box[0]+box[2]>region[0]&&box[1]<region[1]+region[3]&&box[1]+box[3]>region[1])return true;
}
return false;
}
function exposed(node,box){
var top=Math.max(box[1],0);
var bottom=Math.min(box[1]+box[3],44);
if(bottom-top<1)return false;
var x=box[0]+box[2]/2;
var y=top+(bottom-top)/2;
if(x<0||y<0||x>=window.innerWidth||y>=window.innerHeight)return false;
var hit=document.elementFromPoint(x,y);
return !!hit&&(hit===node||node.contains(hit));
}
function handlerTransport(){
var handlers=window.webkit&&window.webkit.messageHandlers;
var handler=handlers&&handlers.a3sDrag;
if(!handler||typeof handler.postMessage!=='function')return null;
return function(drag,block){handler.postMessage(JSON.stringify({drag:drag,block:block}));};
}
function invokeTransport(){
var internals=window.__TAURI_INTERNALS__;
if(!internals||typeof internals.invoke!=='function')return null;
return function(drag,block){internals.invoke('set_window_drag_regions',{drag:drag,block:block}).then(function(){},function(){});};
}
function boot(){
var send=handlerTransport();
if(!send){
tries+=1;
if(tries<40){setTimeout(boot,50);return;}
send=invokeTransport();
if(!send){setTimeout(boot,50);return;}
}
if(window.__A3S_WINDOW_DRAG__)return;
window.__A3S_WINDOW_DRAG__=true;
function publish(){
var drag=[];
var regions=document.querySelectorAll('['+ATTR+']');
for(var i=0;i<regions.length;i++){
var el=regions[i];
var attr=el.getAttribute(ATTR);
if(attr==='false'||!shown(el))continue;
boxes(el,drag);
}
var block=[];
if(drag.length){
var nodes=document.querySelectorAll('a,button,input,select,textarea,label,summary,[contenteditable],[role],[data-ui~="part:message-content"]');
for(var j=0;j<nodes.length;j++){
var node=nodes[j];
if((!clickable(node)&&!messageBody(node))||node.getAttribute(ATTR)!==null||!shown(node))continue;
var found=[];
boxes(node,found);
for(var k=0;k<found.length;k++)if(overlaps(found[k],drag)&&exposed(node,found[k]))block.push(found[k]);
}
}
try{send(drag,block);}catch(e){}
}
var timer=0;
var pending=false;
function schedule(){
if(timer){pending=true;return;}
publish();
timer=setTimeout(function(){timer=0;if(pending){pending=false;schedule();}},100);
}
function start(){
window.addEventListener('resize',schedule);
if(typeof MutationObserver==='function'&&document.documentElement){
new MutationObserver(schedule).observe(document.documentElement,{subtree:true,childList:true,attributes:true,attributeFilter:['style','class','hidden',ATTR]});
}
publish();
}
start();
}
boot();
})();"#;

/// Loopback port of the debug renderer. Unset stays on 4318 so the primary window is unchanged.
pub(crate) fn debug_renderer_port() -> u16 {
    static PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
    *PORT.get_or_init(|| {
        std::env::var("STUDIO_RENDERER_PORT")
            .ok()
            .filter(|raw| {
                !raw.is_empty() && raw.len() <= 5 && raw.chars().all(|ch| ch.is_ascii_digit())
            })
            .and_then(|raw| raw.parse::<u16>().ok())
            .filter(|port| *port > 0)
            .unwrap_or(4318)
    })
}

pub(crate) fn debug_renderer_origin() -> &'static str {
    static ORIGIN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ORIGIN
        .get_or_init(|| format!("http://127.0.0.1:{}", debug_renderer_port()))
        .as_str()
}

fn trusted_host_script_prefix(runtime: &serde_json::Value) -> String {
    // Wry injects main-only scripts into Windows subframes too.
    // Check the executing document before installing host authority or effects.
    let development = cfg!(debug_assertions);
    let renderer_port = debug_renderer_port();
    let base = serde_json::to_string(
        &runtime
            .get("apiBaseUrl")
            .and_then(serde_json::Value::as_str),
    )
    .unwrap_or_else(|_| "null".to_owned());
    format!(
        "(function(){{if(globalThis.top!==globalThis)return;try{{\
         var page=new URL(location.href);var base=new URL({base});\
         if(page.username||page.password||base.username||base.password||\
         base.protocol!=='http:'||base.hostname!=='127.0.0.1'||base.pathname!=='/'||base.search||base.hash)return;\
         var shell=page.origin===base.origin&&(page.pathname==='/'||page.pathname==='/index.html');\
         var main=page.pathname==='/windows/main/index.html'&&(\
         ({development}&&page.protocol==='http:'&&page.host==='127.0.0.1:{renderer_port}')||\
         (page.protocol==='tauri:'&&page.host==='localhost')||\
         ((page.protocol==='http:'||page.protocol==='https:')&&page.host==='tauri.localhost'));\
         if(!shell&&!main)return;}}catch(e){{return;}}"
    )
}

/// The held-turn drive reloads the document from inside the window.
/// Background key chords never arrive in this web view.
pub(crate) fn page_reload_marker(enabled: bool) -> &'static str {
    if enabled {
        "document.documentElement.dataset.a3sPageReload='1';document.documentElement.dataset.a3sBoot=String(Date.now());"
    } else {
        ""
    }
}

fn page_reload_enabled() -> bool {
    std::env::var("A3S_STUDIO_HOLD_TURN").ok().as_deref() == Some("1")
}

pub(crate) fn boot_initialization_script(
    overlay_titlebar: bool,
    reserve_traffic_lights: bool,
    slug: &str,
    product: &str,
    runtime: &serde_json::Value,
    track_drag_regions: bool,
) -> String {
    let slug_js = serde_json::to_string(slug).unwrap_or_else(|_| "\"\"".to_owned());
    let product_js = serde_json::to_string(product).unwrap_or_else(|_| "\"\"".to_owned());
    let host_platform = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else {
        "linux"
    };
    let mut chrome = format!("document.documentElement.dataset.a3sPlatform='{host_platform}';");
    chrome.push_str(page_reload_marker(page_reload_enabled()));
    if overlay_titlebar {
        chrome.push_str("document.documentElement.dataset.overlayTitlebar='true';");
    }
    // Compact lights live inside the 60px icon rail. WKWebView reports
    // env(titlebar-area-*) as 0, so the page reads these variables.
    if reserve_traffic_lights {
        chrome.push_str(
            "document.documentElement.style.setProperty('--a3s-titlebar-height','44px');\
             document.documentElement.style.setProperty('--a3s-traffic-light-inset','60px');",
        );
    }
    chrome.push_str(&crate::host_family::desktop_capabilities_assignment());
    // The signed-in window is the renderer at 127.0.0.1:4318, not the shell
    // origin. It still needs this descriptor to ask the shell for the member's
    // teams. Only the shell document stays hidden until its session check.
    let gate = trusted_host_script_prefix(runtime);
    let mut script = format!(
        "{gate}{chrome}\
         document.documentElement.dataset.tenant={slug_js};\
         document.documentElement.dataset.a3sProduct={product_js};\
         (function(){{var runtime=Object.freeze({runtime});try{{\
         if(shell){{document.documentElement.style.visibility='hidden';}}\
         if(shell||main){{Object.defineProperty(globalThis,'__A3S_STUDIO_RUNTIME__',{{value:runtime,configurable:false,enumerable:false,writable:false}});}}\
         }}catch(e){{}}}})();"
    );
    if track_drag_regions {
        script.push_str(MACOS_WINDOW_DRAG_SCRIPT);
    }
    script.push_str("})();");
    script
}

pub(crate) fn is_main_renderer_document(loaded: &str) -> bool {
    let Ok(page) = url::Url::parse(loaded) else {
        return false;
    };
    if !page.username().is_empty()
        || page.password().is_some()
        || page.path() != "/windows/main/index.html"
    {
        return false;
    }
    match (page.scheme(), page.host_str(), page.port()) {
        ("http", Some("127.0.0.1"), Some(port)) if port == debug_renderer_port() => {
            cfg!(debug_assertions)
        }
        ("tauri", Some("localhost"), None) => true,
        ("http" | "https", Some("tauri.localhost"), None) => true,
        _ => false,
    }
}

/// Runs after the login page assigns the main renderer. The document-start
/// user script does not run on that navigation, so the runtime descriptor is
/// missing and an inline `visibility: hidden` left by the shell would keep
/// the white boot layer up.
pub(crate) fn main_renderer_resume_script(
    runtime: &serde_json::Value,
    slug: &str,
    product: &str,
) -> String {
    let slug_js = serde_json::to_string(slug).unwrap_or_else(|_| "\"\"".to_owned());
    let product_js = serde_json::to_string(product).unwrap_or_else(|_| "\"\"".to_owned());
    let gate = trusted_host_script_prefix(runtime);
    let caps = crate::host_family::desktop_capabilities_assignment();
    format!(
        "{gate}{caps}var runtime=Object.freeze({runtime});try{{\
         if(!Object.prototype.hasOwnProperty.call(globalThis,'__A3S_STUDIO_RUNTIME__')){{\
         Object.defineProperty(globalThis,'__A3S_STUDIO_RUNTIME__',{{value:runtime,configurable:false,enumerable:false,writable:false}});}}\
         }}catch(e){{}}\
         document.documentElement.dataset.tenant={slug_js};\
         document.documentElement.dataset.a3sProduct={product_js};\
         if(document.documentElement.style.visibility==='hidden'){{document.documentElement.style.visibility='visible';}}\
         try{{window.dispatchEvent(new Event('a3s-studio-boot'));}}catch(e){{}}\
         {MACOS_WINDOW_DRAG_SCRIPT}\
         }})();"
    )
}

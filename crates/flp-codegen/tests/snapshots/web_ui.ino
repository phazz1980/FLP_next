// Сгенерировано FLP_next из проекта «web_ui». Ручные правки будут перезаписаны.
// Порядок выполнения: wifi → ui → head → sw → btn → sld → inp → val → bar → any → lamp → led → txt → info → line → box → trend → net → tank → clock → pic → fan → arrow → panel → sepv

// Tells the web page block to add the network form handlers (/ui/net, /ui/scan).
#define FLP_WEB_NET

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>
#include <Preferences.h>
#include <WiFi.h>

// Выходы блоков
bool wifi_ready = false;
String wifi_address = String();
bool sw_out = false;
bool btn_out = false;
float sld_out = 0.0f;
float inp_out = 0.0f;
bool any_out = false;
String txt_out = String();
String net_ssid = String();
String net_password = String();
bool net_dhcp = false;
String net_ip = String();
String net_gateway = String();
String net_subnet = String();
int32_t net_mode = 0;
bool net_changed = false;

// web_ui: общий код
// Web interface registry: widget blocks register their items, the web_ui block serves them.
struct FlpUiItem {
  const char *kind;
  const char *caption;
  const char *unit;
  uint8_t col, row, w, h;
  uint8_t page; // 1-based page number
  bool rw;      // the page may change the value
  float value;
  float lo, hi, step; // for a chart `step` is the sampling period in seconds
  uint8_t dec;  // digits after the point on the page
  String text;
  float *hist;  // chart history (ring buffer of hn points), null for other widgets
  uint16_t hn, hpos, hfill;
  uint32_t hlast;
  const char *color; // CSS colour of the widget, empty/null: default
  const char *back;  // CSS colour of the tile background
};
#define FLP_UI_MAX 32
FlpUiItem *flpUiItems[FLP_UI_MAX];
uint8_t flpUiCount = 0;
void flpUiAdd(FlpUiItem *item) {
  if (flpUiCount < FLP_UI_MAX) flpUiItems[flpUiCount++] = item;
}
// Chart widgets call this every loop: a point is stored once per period.
void flpUiSample(FlpUiItem *it) {
  uint32_t now = millis();
  if (it->hfill && now - it->hlast < (uint32_t)it->step * 1000UL) return;
  it->hlast = now;
  float v = it->value;
  if (isnan(v) || isinf(v)) v = 0;
  it->hist[it->hpos] = v;
  it->hpos = (it->hpos + 1) % it->hn;
  if (it->hfill < it->hn) it->hfill++;
}
#define FLP_WEBUI 1

// web_net: общий код
// Network settings shared with the Wi-Fi and Ethernet blocks: ssid, password, dhcp, ip, gateway, subnet, ap ssid, ap password, mode, ap delay.
struct FlpNet {
  String f[10];
  // Mode actually applied: differs from f[8] in the automatic access point mode.
  int eff = 0;
};
FlpNet flpNet;

// wifi: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface flpWifi;
// Applied radio state: 0 off, 1 client, 2 access point, 3 both; 255 not yet.
uint8_t wifi_state = 255;
String wifi_cfg;
// Address shown on the address output; the text is rebuilt only when it changes.
uint32_t wifi_ip_raw = 0;
// Web server (port 80): the web_ui block adds its pages to it.
FLProgWebServer wifi_web(&flpWifi, 80);
// Shared web server: the web_ui block adds its pages to it.
FLProgWebServer &flpWebSrv = wifi_web;
// Update over the network (ArduinoOTA through the shared interface).
FLProgNetworkOta wifi_ota;
bool wifi_ota_on = false;
#define FLP_WEB_UPD 1
// Firmware upload from the web page: raw HTTP POST streamed into Update (port 8080).
FLProgNetworkTcpServer wifi_up;
FLProgNetworkTcpServerClient *wifi_up_c = nullptr;
bool wifi_up_on = false;
uint8_t wifi_up_st = 0;  // 0 idle, 1 headers, 2 body, 3 done (restart pending)
char wifi_up_line[160];
uint8_t wifi_up_ll = 0;
uint32_t wifi_up_len = 0, wifi_up_got = 0, wifi_up_t = 0, wifi_up_try = 0;
bool wifi_up_auth = false;

// ui: web_ui
const char ui_html[] PROGMEM = R"FLPUI(<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>FLP_next</title>
<style>
:root{color-scheme:light dark;--b:#fff;--f:#222;--c:#f1f3f6;--a:#2f7de1;--g:#2da44e}
@media(prefers-color-scheme:dark){:root{--b:#14171c;--f:#e6e8eb;--c:#222831;--a:#5aa0f5;--g:#3fb950}}
body{margin:0;background:var(--b);color:var(--f);font:15px system-ui,sans-serif;display:flex;min-height:100vh}
#n{flex:none;width:200px;box-sizing:border-box;padding:12px 8px;background:var(--c);display:flex;flex-direction:column;gap:4px;position:sticky;top:0;height:100vh;overflow:auto}
#n a{display:flex;align-items:center;gap:10px;padding:8px 10px;border-radius:8px;color:inherit;text-decoration:none;white-space:nowrap}
#n a i{flex:none;width:26px;height:26px;border-radius:50%;background:#8884;font-style:normal;text-align:center;line-height:26px;font-size:13px}
#n a.on{background:var(--a);color:#fff}
#n a.on i{background:#fff4}
[hidden]{display:none!important}
#m{flex:1;min-width:0;padding:12px}
h1{font-size:18px;margin:0 0 10px}
@media(max-width:900px){ #n{width:64px}#n a{justify-content:center;padding:8px 0}#n a span{display:none}}
@media(max-width:600px){body{flex-direction:column}#n{width:auto;height:auto;position:static;flex-direction:row;overflow-x:auto;padding:8px}#n a{padding:6px 12px}#n a span{display:inline}#n a i{display:none}}
#gs>div{display:grid;grid-template-columns:repeat(24,1fr);grid-auto-rows:24px;gap:8px}
#gs>div.sy{display:block;max-width:560px}
.w{background:var(--c);border-radius:8px;padding:6px 10px;display:flex;flex-direction:column;justify-content:center;min-width:0;overflow:hidden}
.w small{opacity:.7;font-size:12px}
.w b{font-size:22px;font-weight:600}
.w.p{padding:0}
.w.cht{justify-content:flex-start}
.cv{flex:1;min-height:0;position:relative}
.cv canvas{position:absolute;left:0;top:0;width:100%;height:100%}
button,input{font:inherit;color:inherit}
.btn{border:0;border-radius:8px;background:var(--a);color:#fff;width:100%;height:100%;touch-action:manipulation;user-select:none}
.btn.on{background:var(--g)}
.led{width:22px;height:22px;border-radius:50%;background:#888;flex:none}
.led.on{background:var(--g);box-shadow:0 0 8px var(--g)}
.r{display:flex;align-items:center;gap:8px}
.bar{height:10px;border-radius:5px;background:#8884;overflow:hidden;margin-top:4px}
.bar i{display:block;height:100%;background:var(--a)}
.ic0{align-items:center}
.ic{flex:1;min-height:0;width:100%;color:#888;transition:color .2s}
.ic.on{color:var(--g)}
.ic.sp svg{animation:spn 1.2s linear infinite}
@keyframes spn{to{transform:rotate(360deg)}}
.w.nw{overflow:auto;gap:6px;padding:10px 14px}
.nw h4{margin:8px 0 0;font-size:12px;text-transform:uppercase;letter-spacing:.06em;opacity:.6;font-weight:600}
.nr{display:grid;grid-template-columns:minmax(110px,38%) 1fr;align-items:center;gap:10px}
.nr small{opacity:.85}
.nr input[type=checkbox]{justify-self:start;width:18px;height:18px}
.nr select,.nw select{width:100%;box-sizing:border-box;background:transparent;color:inherit;border:1px solid #8886;border-radius:6px;padding:4px}
.nr select option,.nw select option{color:#222}
.nr input:disabled,.nr select:disabled{opacity:.45}
.nb{display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin-top:6px}
.nb .bt{margin-top:0}.nb select{flex:1;min-width:140px;width:auto}
.tk{width:44px;align-self:stretch;min-height:40px;border-radius:6px;background:#8884;overflow:hidden;display:flex;flex-direction:column-reverse;flex:none}
.tk i{display:block;width:100%;background:var(--a)}
.w.tkw .r{flex:1;min-height:0}
input[type=range]{width:100%}
input[type=number],input[type=password],input[type=text]{width:100%;box-sizing:border-box;background:transparent;border:1px solid #8886;border-radius:6px;padding:4px}
table{border-collapse:collapse;width:100%}
td{padding:6px 8px;border-bottom:1px solid #8883}
td:first-child{opacity:.7}
.bt{border:0;border-radius:8px;background:var(--a);color:#fff;padding:8px 14px;margin-top:8px}
.btn,.bt{transition:transform .12s,filter .12s,background-color .2s}
.btn:active,.bt:active{transform:scale(.96);filter:brightness(.88)}
.led{transition:background-color .2s,box-shadow .2s}
.bar i{transition:width .3s}
.tk i{transition:height .3s}
@media(prefers-reduced-motion:reduce){.btn,.bt,.led,.bar i,.tk i,.ic{transition:none}.ic.sp svg{animation:none}.btn:active,.bt:active{transform:none}}
#lg{max-width:320px;margin:60px auto;display:flex;flex-direction:column;gap:8px}
</style></head><body><nav id="n" hidden></nav><div id="m"><h1 id="t"></h1><div id="gs"></div></div><script>
var GS=document.getElementById('gs'),N=document.getElementById('n'),T=document.getElementById('t'),M=document.getElementById('m'),E=[],P=[],A=[],K='';
try{K=localStorage.getItem('flpk')||''}catch(e){}
function F(u,o){return fetch(u+(u.indexOf('?')<0?'?':'&')+'k='+encodeURIComponent(K),o)}
function S(i,v){F('/ui/set?i='+i+'&v='+v).catch(function(){})}
function h(t,c,x){var e=document.createElement(t);if(c)e.className=c;if(x!=null)e.textContent=x;return e}
function mk(d,i){
 var w=h('div','w'),k=d.k,u=d.u?' '+d.u:'',dn=false;
 w.style.gridColumn=d.c+' / span '+d.w;w.style.gridRow=d.r+' / span '+d.h;
 if(d.q)w.style.background=d.q;
 if(d.o){w.style.setProperty('--a',d.o);if(k!='button'&&k!='switch')w.style.setProperty('--g',d.o)}
 if(k=='shape'){
  var s=h('div'),t=Math.max(1,d.b||2),c=d.o||'currentColor',m=d.a;if(m!=10){w.style.background='none';w.style.padding='0'}
  s.style.boxSizing='border-box';
  if(m==0){s.style.borderTop=t+'px solid '+c;s.style.width='100%'}
  else if(m==1){s.style.borderLeft=t+'px solid '+c;s.style.height='100%';s.style.alignSelf='center'}
  else if(m==8){s.style.borderTop='1px solid '+c;s.style.opacity='.4';s.style.width='100%'}
  else if(m==9){s.style.borderLeft='1px solid '+c;s.style.opacity='.4';s.style.height='100%';s.style.alignSelf='center'}
  else if(m==10){w.style.background=d.q||'';w.style.padding='6px 10px';w.style.justifyContent='flex-start';if(d.l)w.appendChild(h('small','',d.l));return[w,function(){}]}
  else if(m>=4&&m<=6){var P=['M0 50H78M58 24L96 50L58 76','M50 0V78M24 58L50 96L76 58','M50 6L96 94H4Z'];
   s.style.width=s.style.height='100%';s.innerHTML='<svg viewBox="0 0 100 100" preserveAspectRatio="none" width="100%" height="100%"><path d="'+P[m-4]+'" fill="'+(m==6?(d.q||'none'):'none')+'" stroke="'+c+'" stroke-width="'+t+'" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke"/></svg>'}
  else{s.style.border=t+'px solid '+c;s.style.width=s.style.height='100%';s.style.background=d.q||'none';s.style.borderRadius=m==3?'50%':(m==7?'24px':'8px')}
  w.appendChild(s);return[w,function(){}]}
 if(k=='label'){w.appendChild(h('b','',d.l));return[w,function(){}]}
 if(k=='button'||k=='switch'){
  var b=h('button','btn',d.l);w.className='w p';w.appendChild(b);
  if(k=='switch')b.onclick=function(){var n=b.classList.contains('on')?0:1;b.classList.toggle('on',n==1);S(i,n)};
  else{
   b.onpointerdown=function(){dn=true;S(i,1)};
   b.onpointerup=b.onpointerleave=b.onpointercancel=function(){if(dn){dn=false;S(i,0)}}}
  return[w,function(v){b.classList.toggle('on',v!=0)}]}
 if(k=='slider'){
  var s=h('input'),o=h('b','','');s.type='range';s.min=d.a;s.max=d.b;s.step=d.s;
  w.appendChild(h('small','',d.l));w.appendChild(o);w.appendChild(s);
  s.oninput=function(){o.textContent=s.value+u;S(i,s.value)};
  return[w,function(v){if(document.activeElement!=s){s.value=v;o.textContent=(+v)+u}}]}
 if(k=='input'){
  var n=h('input');n.type='number';n.step=d.s;if(d.b>d.a){n.min=d.a;n.max=d.b}
  w.appendChild(h('small','',d.l+u));w.appendChild(n);
  n.onchange=function(){S(i,n.value)};
  return[w,function(v){if(document.activeElement!=n)n.value=v}]}
 if(k=='net'){
  var nf={},nm=[['o','Режим Wi-Fi','select'],['s','Сеть Wi-Fi','text'],['p','Пароль (пусто — не менять)','password'],['d','Адрес по DHCP','checkbox'],['i','IP-адрес','text'],['g','Шлюз','text'],['m','Маска','text'],['a','Своя точка доступа','text'],['b','Пароль точки (пусто — не менять)','password'],['w','Задержка точки без связи, с','number']],bt=h('button','bt','Сохранить'),ms=h('small','',''),sc=h('button','bt','Сканировать'),sl=h('select');
  w.className='w cht nw';w.appendChild(h('b','',d.l));
  nm.forEach(function(r){var l=h('label','r'),e=r[2]=='select'?h('select'):h('input');
   if(r[2]=='select')['Клиент','Точка доступа','Клиент и точка','Клиент, при отсутствии связи — точка'].forEach(function(t,j){e.appendChild(new Option(t,j))});else e.type=r[2];
   if(r[0]=='o')w.appendChild(h('h4','','Подключение'));if(r[0]=='a')w.appendChild(h('h4','','Точка доступа'));
   l.className='nr';l.appendChild(h('small','',r[1]));l.appendChild(e);w.appendChild(l);nf[r[0]]=e});
  function ds(){['i','g','m'].forEach(function(c){nf[c].disabled=nf.d.checked});
   var m=+nf.o.value;['s','p'].forEach(function(c){nf[c].disabled=m==1});['a','b'].forEach(function(c){nf[c].disabled=m==0});nf.w.disabled=m!=3;sc.disabled=m==1}
  nf.d.onchange=nf.o.onchange=ds;
  function chk(){var e='';
   [['s','Сеть Wi-Fi'],['a','Точка доступа']].forEach(function(r){var v=nf[r[0]].value;if(new Blob([v]).size>32)e=r[1]+': имя не длиннее 32 байт'});
   [['p','Пароль'],['b','Пароль точки']].forEach(function(r){var v=nf[r[0]].value;
    if(v&&!/^[ -~]*$/.test(v))e=r[1]+': недопустимые символы (только латиница, цифры и знаки ASCII)';
    else if(v&&!(v.length>=8&&v.length<=63)&&!/^[0-9a-fA-F]{64}$/.test(v))e=r[1]+': нужно 8-63 символа'});
   return e}
  bt.onclick=function(){var er=chk();if(er){ms.textContent=er;return}var q='';nm.forEach(function(r){q+='&'+r[0]+'='+encodeURIComponent(r[0]=='d'?(nf.d.checked?'1':'0'):nf[r[0]].value)});
   F('/ui/net?i='+i+q).then(function(){ms.textContent='Сохранено и применено';nf.p.value=nf.b.value=''}).catch(function(){ms.textContent='Ошибка'})};
  sl.style.display='none';sl.onchange=function(){if(sl.value)nf.s.value=sl.value};
  function scn(){F('/ui/scan').then(function(r){return r.text()}).then(function(t){
   if(t=='?'){setTimeout(scn,1500);return}
   sl.innerHTML='';sl.appendChild(new Option('— найденные сети —',''));
   t.split('\n').forEach(function(x){var p=x.split('\t');if(p[0])sl.appendChild(new Option(p[0]+' ('+p[1]+' дБм)',p[0]))});
   sl.style.display='';sc.disabled=false;ms.textContent='Найдено сетей: '+(sl.options.length-1)}).catch(function(){sc.disabled=false;ms.textContent='Ошибка сканирования'})}
  sc.onclick=function(){sc.disabled=true;ms.textContent='Сканирование...';scn()};
  var br=h('div','nb');br.appendChild(sc);br.appendChild(sl);br.appendChild(bt);w.appendChild(br);w.appendChild(ms);
  return[w,function(v){if(w.contains(document.activeElement))return;var a=String(v).split('\n');
   nf.s.value=a[0]||'';nf.d.checked=a[2]!='0';nf.i.value=a[3]||'';nf.g.value=a[4]||'';nf.m.value=a[5]||'';nf.a.value=a[6]||'';nf.o.value=a[8]||'0';nf.w.value=a[9]||'30';ds()}]}
 if(k=='tank'){
  var q=h('b','',''),a=h('i'),z=h('div','tk'),r=h('div','r');w.className='w tkw';z.appendChild(a);
  w.appendChild(h('small','',d.l));r.appendChild(z);r.appendChild(q);w.appendChild(r);
  return[w,function(v){q.textContent=(+v).toFixed(d.d)+u;a.style.height=Math.max(0,Math.min(100,(v-d.a)/(d.b-d.a)*100))+'%'}]}
 if(k=='clock'){
  var q=h('b','',''),e=h('small','','');w.appendChild(h('small','',d.l));w.appendChild(q);w.appendChild(e);
  function tk(){var t=new Date(),p=function(n){return(n<10?'0':'')+n};
   q.textContent=p(t.getHours())+':'+p(t.getMinutes())+(d.a?':'+p(t.getSeconds()):'');
   e.textContent=d.b?t.toLocaleDateString(undefined,{weekday:'short',day:'numeric',month:'long',year:'numeric'}):''}
  tk();setInterval(tk,1000);return[w,function(){}]}
 if(k=='image'){
  var m=h('img');m.src=d.u;m.alt=d.l;m.style.cssText='width:100%;flex:1;min-height:0;object-fit:'+(d.a?'contain':'cover');
  if(d.l)w.appendChild(h('small','',d.l));w.appendChild(m);return[w,function(){}]}
 if(k=='icon'){
  var I=['<circle cx="32" cy="32" r="22"/><path d="M32 10V4M10 32H4M54 32h6"/><path d="M10 54h44"/>','<circle cx="32" cy="32" r="4"/><path d="M32 28C32 8 46 8 46 18C46 26 36 28 32 28zM36 32C56 32 56 46 46 46C38 46 36 36 36 32zM32 36C32 56 18 56 18 46C18 38 28 36 32 36zM28 32C8 32 8 18 18 18C26 18 28 28 28 32z"/>','<path d="M8 12L32 32L8 52zM56 12L32 32L56 52z"/><path d="M32 32V8"/>','<path d="M32 6C32 6 14 28 14 40a18 18 0 0 0 36 0C50 28 32 6 32 6z"/>','<path d="M32 4C36 20 50 26 50 42a18 18 0 0 1-36 0C14 34 20 30 22 22C26 28 30 26 32 4z"/>','<path d="M32 6a14 14 0 0 0-14 14c0 12-8 14-8 24h44c0-10-8-12-8-24A14 14 0 0 0 32 6zM26 52a6 6 0 0 0 12 0"/>','<path d="M32 4V60M8 18L56 46M8 46L56 18"/>','<rect x="12" y="28" width="40" height="30" rx="4"/><path d="M20 28V20a12 12 0 0 1 24 0v8"/>','<path d="M24 8a8 8 0 0 1 16 0V36a14 14 0 1 1-16 0z"/>','<path d="M4 44c8-8 16-8 28 0s20 8 28 0M4 28c8-8 16-8 28 0s20 8 28 0"/>'];
  var sv=h('div','ic'),n=Math.max(0,Math.min(I.length-1,d.a|0));
  sv.innerHTML='<svg viewBox="0 0 64 64" width="100%" height="100%" fill="none" stroke="currentColor" stroke-width="3" stroke-linejoin="round" stroke-linecap="round">'+I[n]+'</svg>';
  w.className='w ic0';if(d.l)w.appendChild(h('small','',d.l));w.appendChild(sv);
  return[w,function(v){sv.classList.toggle('on',v!=0);sv.classList.toggle('sp',v!=0&&(n==1||n==6))}]}
 if(k=='led'){
  var r=h('div','r'),l=h('div','led');r.appendChild(l);r.appendChild(h('span','',d.l));w.appendChild(r);
  return[w,function(v){l.classList.toggle('on',v!=0)}]}
 if(k=='gauge'){
  var q=h('b','',''),a=h('i'),z=h('div','bar');z.appendChild(a);
  w.appendChild(h('small','',d.l));w.appendChild(q);w.appendChild(z);
  return[w,function(v){q.textContent=(+v).toFixed(d.d)+u;a.style.width=Math.max(0,Math.min(100,(v-d.a)/(d.b-d.a)*100))+'%'}]}
 if(k=='chart'){
  var y=h('b','',''),cv=h('div','cv'),c=h('canvas'),H=[];w.className='w cht';
  cv.appendChild(c);w.appendChild(h('small','',d.l));w.appendChild(y);w.appendChild(cv);
  function dr(){
   var W=c.width=c.clientWidth,Q=c.height=c.clientHeight,x=c.getContext('2d'),lo=d.a,hi=d.b;
   if(!(hi>lo)&&H.length){lo=Math.min.apply(null,H);hi=Math.max.apply(null,H);if(hi==lo){lo-=1;hi+=1}else{var p=(hi-lo)*.1;lo-=p;hi+=p}}
   var st=getComputedStyle(document.body),f=st.color,L=34;
   x.font='11px system-ui';x.fillStyle=f;x.strokeStyle='#8883';x.lineWidth=1;
   for(var j=0;j<=2;j++){var yy=4+(Q-8)*j/2;x.beginPath();x.moveTo(L,yy);x.lineTo(W,yy);x.stroke();
    x.fillText((hi-(hi-lo)*j/2).toFixed(Math.abs(hi-lo)<10?1:0),0,yy+4)}
   if(H.length<2)return;
   x.strokeStyle=getComputedStyle(w).getPropertyValue('--a');x.lineWidth=2;x.beginPath();
   H.forEach(function(v,j){var px=L+(W-L)*j/(H.length-1),py=4+(Q-8)*(1-(v-lo)/(hi-lo));j?x.lineTo(px,py):x.moveTo(px,py)});x.stroke()}
  function ld(){F('/ui/hist?i='+i).then(function(r){return r.json()}).then(function(a){H=a;dr()}).catch(function(){})}
  setInterval(ld,Math.max(1000,d.s*1000));ld();window.addEventListener('resize',dr);
  return[w,function(v){y.textContent=(+v).toFixed(d.d)+u}]}
 var x=h('b','','');w.appendChild(h('small','',d.l));w.appendChild(x);
 return[w,function(v){x.textContent=(k=='value'?(+v).toFixed(d.d)+u:v)}]}
function sy(){
 var g=h('div','sy'),t=h('table'),R={},rows=[['ip','IP-адрес'],['mac','MAC'],['rssi','Сигнал Wi-Fi'],['up','Аптайм'],['chip','Чип'],['heap','Память (свободно)'],['flash','Флеш'],['app','Прошивка'],['build','Сборка']];
 rows.forEach(function(r){var tr=h('tr'),v=h('td','','');tr.appendChild(h('td','',r[1]));tr.appendChild(v);t.appendChild(tr);R[r[0]]=v});
 g.appendChild(t);var up=h('div');up.hidden=true;g.appendChild(up);
 var f=h('input'),bt=h('button','bt','Загрузить прошивку'),st=h('div');f.type='file';f.accept='.bin';
 up.appendChild(h('h2','','Обновление прошивки'));up.appendChild(f);up.appendChild(h('br'));up.appendChild(bt);up.appendChild(st);
 bt.onclick=function(){var b=f.files[0];if(!b)return;var p=prompt('Пароль обновления','');st.textContent='...';
  fetch('http://'+location.hostname+':8080/update?p='+encodeURIComponent(p||''),{method:'POST',headers:{'Content-Type':'text/plain'},body:b}).then(function(r){return r.text()}).then(function(x){st.textContent=x}).catch(function(){st.textContent='ошибка'})};
 function ld(){if(g.hidden)return;F('/ui/info').then(function(r){return r.json()}).then(function(d){
  var s=d.up;R.ip.textContent=d.ip;R.mac.textContent=d.mac;R.rssi.textContent=d.rssi+' дБм';
  R.up.textContent=Math.floor(s/86400)+' д '+Math.floor(s/3600)%24+' ч '+Math.floor(s/60)%60+' мин';
  R.chip.textContent=d.chip+', '+d.cpu+' МГц';R.heap.textContent=Math.round(d.heap/1024)+' из '+Math.round(d.heapAll/1024)+' КБ';
  R.flash.textContent=Math.round(d.flash/1048576)+' МБ';R.app.textContent=Math.round(d.sketch/1024)+' КБ, свободно '+Math.round(d.free/1024)+' КБ';
  R.build.textContent=d.build;up.hidden=!d.u}).catch(function(){})}
 setInterval(ld,2000);g.onshow=ld;return g}
function poll(){F('/ui/state').then(function(r){return r.json()}).then(function(a){a.forEach(function(v,i){if(E[i])E[i](v)})}).catch(function(){}).then(function(){setTimeout(poll,500)})}
function go(n){P.forEach(function(g,j){g.hidden=j!=n});A.forEach(function(a,j){a.classList.toggle('on',j==n)});history.replaceState(null,'','#'+(n+1));if(P[n].onshow)P[n].onshow()}
function lg(bad){
 M.textContent='';var b=h('div');b.id='lg';b.appendChild(h('h1','','Вход'));
 var p=h('input');p.type='password';p.placeholder='Пароль';var o=h('button','bt','Войти');
 o.onclick=function(){K=p.value;try{localStorage.setItem('flpk',K)}catch(e){}location.reload()};
 p.onkeydown=function(e){if(e.key=='Enter')o.onclick()};
 b.appendChild(p);b.appendChild(o);if(bad)b.appendChild(h('div','','Неверный пароль'));M.appendChild(b);p.focus()}
F('/ui/layout').then(function(r){if(r.status==403){lg(K!='');throw 0}return r.json()}).then(function(d){
 T.textContent=d.t;document.title=d.t;if(d.bg)document.body.style.background=d.bg;
 var nm=d.p.split(';');
 function tab(s,g){g.hidden=true;GS.appendChild(g);P.push(g);var j=P.length-1;
  var a=h('a');a.href='#'+(j+1);a.title=s;a.appendChild(h('i','',s.charAt(0).toUpperCase()));a.appendChild(h('span','',s));
  a.onclick=function(e){e.preventDefault();go(j)};N.appendChild(a);A.push(a)}
 nm.forEach(function(s){tab(s,h('div'))});
 if(d.y)tab('Система',sy());
 N.hidden=P.length<2;
 d.i.forEach(function(x,i){var m=mk(x,i);P[Math.min(Math.max(x.g,1),nm.length)-1].appendChild(m[0]);E.push(m[1])});
 go(Math.min(Math.max(parseInt(location.hash.slice(1))||1,1),P.length)-1);poll()}).catch(function(){})
</script></body></html>)FLPUI";

// head: web_label
FlpUiItem head_ui = {"web_label", "Управление", "", 1, 1, 24, 2, 1, false, 0, 0, 0, 0, 0};

// sw: web_switch
FlpUiItem sw_ui = {"web_switch", "Свет", "", 1, 3, 8, 2, 1, true, 0, 0, 0, 0, 0};

// btn: web_button
FlpUiItem btn_ui = {"web_button", "Нажать", "", 9, 3, 8, 2, 1, true, 0, 0, 0, 0, 0};

// sld: web_slider
FlpUiItem sld_ui = {"web_slider", "Ползунок", "%", 1, 5, 16, 2, 1, true, 25.0f, 0.0f, 100.0f, 5.0f, 3};

// inp: web_input
FlpUiItem inp_ui = {"web_input", "Уставка", "°C", 17, 5, 8, 2, 1, true, 21.5f, -50.0f, 150.0f, 0.5f, 3};

// val: web_value
FlpUiItem val_ui = {"web_value", "Ползунок", "%", 1, 7, 8, 4, 1, false, 0, 0, 0, 0, 0};

// bar: web_gauge
FlpUiItem bar_ui = {"web_gauge", "Шкала", "%", 9, 7, 16, 4, 1, false, 0, 0.0f, 100.0f, 0, 1};

// lamp: web_led
FlpUiItem lamp_ui = {"web_led", "Светодиод", "", 17, 3, 8, 2, 1, false, 0, 0, 0, 0, 0};

// info: web_text
FlpUiItem info_ui = {"web_text", "Состояние", "", 1, 11, 24, 2, 1, false, 0, 0, 0, 0, 0};

// line: web_shape
// The shape code travels in `lo` and the thickness in `hi` (a shape has no value).
FlpUiItem line_ui = {"web_shape", "", "", 1, 13, 24, 2, 1, false, 0, 0, 2, 0, 0};

// box: web_shape
// The shape code travels in `lo` and the thickness in `hi` (a shape has no value).
FlpUiItem box_ui = {"web_shape", "", "", 1, 15, 4, 4, 1, false, 0, 3, 3, 0, 0};

// trend: web_chart
float trend_buf[60];
FlpUiItem trend_ui = {"web_chart", "Ползунок", "%", 5, 15, 20, 8, 1, false, 0, 0.0f, 100.0f, 2, 1, String(), trend_buf, 60};

// net: web_net
FlpUiItem net_ui = {"web_net", "Настройки сети", "", 1, 23, 16, 10, 1, true, 0, 0, 0, 0, 0};
Preferences net_prefs;

// tank: web_tank
FlpUiItem tank_ui = {"web_tank", "Ёмкость", "%", 1, 33, 4, 8, 1, false, 0, 0.0f, 100.0f, 0, 0};

// clock: web_clock
FlpUiItem clock_ui = {"web_clock", "Время", "", 5, 33, 8, 4, 1, false, 0, 1, 1, 0, 0};

// pic: web_image
// The image address travels in `unit`; the fit mode in `lo`.
FlpUiItem pic_ui = {"web_image", "Схема", "https://upload.wikimedia.org/wikipedia/commons/thumb/b/b0/ESP32_Espressif_ESP-WROOM-32_Wi-Fi_Bluetooth_Module.jpg/320px-ESP32_Espressif_ESP-WROOM-32_Wi-Fi_Bluetooth_Module.jpg", 13, 33, 8, 8, 1, false, 0, 1, 0, 0, 0};

// fan: web_icon
// The icon number travels in `lo`.
FlpUiItem fan_ui = {"web_icon", "Вентилятор", "", 21, 33, 4, 6, 1, false, 0, 1, 0, 0, 0};

// arrow: web_shape
// The shape code travels in `lo` and the thickness in `hi` (a shape has no value).
FlpUiItem arrow_ui = {"web_shape", "", "", 1, 41, 8, 3, 1, false, 0, 4, 3, 0, 0};

// panel: web_shape
// The shape code travels in `lo` and the thickness in `hi` (a shape has no value).
FlpUiItem panel_ui = {"web_shape", "Насос", "", 9, 41, 16, 6, 1, false, 0, 10, 2, 0, 0};

// sepv: web_shape
// The shape code travels in `lo` and the thickness in `hi` (a shape has no value).
FlpUiItem sepv_ui = {"web_shape", "", "", 12, 42, 1, 4, 1, false, 0, 9, 2, 0, 0};

// wifi: wifi
void wifi_up_reply(const char *status, const char *text) {
  String r = String("HTTP/1.1 ") + status + "\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\nContent-Length: " + String(strlen(text)) + "\r\n\r\n" + text;
  wifi_up_c->write((const uint8_t *)r.c_str(), r.length());
}

void wifi_up_fail(const char *status, const char *text) {
  if (wifi_up_st == 2) {
    Update.abort();
  }
  wifi_up_reply(status, text);
  wifi_up_c->stop();
  wifi_up_c = nullptr;
  wifi_up_st = 0;
}

// Header line: the request line is checked for path and password, Content-Length is remembered.
void wifi_up_header_line() {
  char *l = wifi_up_line;
  if (strncmp(l, "POST /update", 12) == 0) {
    String expect = String("?p=") + "" + " ";
    wifi_up_auth = strlen("") == 0 || strstr(l, expect.c_str()) != nullptr;
  } else if (strncasecmp(l, "Content-Length:", 15) == 0) {
    wifi_up_len = (uint32_t)strtoul(l + 15, nullptr, 10);
  }
}

void wifi_up_poll() {
  if (!wifi_up_on) {
    if (millis() - wifi_up_try > 2000) {
      wifi_up_try = millis();
      wifi_up_on = wifi_up.begin(flpWifi, 8080);
    }
    return;
  }
  wifi_up.pool();
  if (wifi_up_c == nullptr) {
    wifi_up_c = wifi_up.takeNewClient();
    if (wifi_up_c == nullptr) {
      return;
    }
    wifi_up_st = 1;
    wifi_up_ll = 0;
    wifi_up_len = 0;
    wifi_up_got = 0;
    wifi_up_auth = false;
    wifi_up_t = millis();
  }
  if (wifi_up_st == 3) {
    if (millis() - wifi_up_t > 800) {
      ESP.restart();
    }
    return;
  }
  if (!wifi_up_c->active() || millis() - wifi_up_t > 10000) {
    if (wifi_up_st == 2) {
      Update.abort();
    }
    wifi_up_c->stop();
    wifi_up_c = nullptr;
    wifi_up_st = 0;
    return;
  }
  if (wifi_up_st == 1) {
    for (uint16_t n = 0; n < 512 && wifi_up_st == 1; n++) {
      int b = wifi_up_c->read();
      if (b < 0) {
        break;
      }
      wifi_up_t = millis();
      if (b == '\n') {
        wifi_up_line[wifi_up_ll] = 0;
        if (wifi_up_ll > 0 && wifi_up_line[wifi_up_ll - 1] == '\r') {
          wifi_up_line[wifi_up_ll - 1] = 0;
        }
        bool empty = wifi_up_line[0] == 0;
        if (!empty) {
          wifi_up_header_line();
        }
        wifi_up_ll = 0;
        if (empty) {
          if (!wifi_up_auth) {
            wifi_up_fail("403 Forbidden", "Неверный пароль");
          } else if (wifi_up_len == 0 || !Update.begin(wifi_up_len)) {
            wifi_up_fail("500 Internal Server Error", "Нет места под прошивку");
          } else {
            wifi_up_st = 2;
          }
        }
      } else if (wifi_up_ll < sizeof(wifi_up_line) - 1) {
        wifi_up_line[wifi_up_ll++] = (char)b;
      }
    }
  }
  if (wifi_up_st == 2) {
    for (uint8_t n = 0; n < 4 && wifi_up_got < wifi_up_len; n++) {
      uint8_t buf[512];
      uint32_t want = wifi_up_len - wifi_up_got;
      int got = wifi_up_c->read(buf, want < sizeof(buf) ? want : sizeof(buf));
      if (got <= 0) {
        break;
      }
      wifi_up_t = millis();
      if (Update.write(buf, (size_t)got) != (size_t)got) {
        wifi_up_fail("500 Internal Server Error", "Ошибка записи прошивки");
        return;
      }
      wifi_up_got += (uint32_t)got;
    }
    if (wifi_up_got >= wifi_up_len) {
      if (Update.end(true)) {
        wifi_up_reply("200 OK", "Прошивка загружена, устройство перезагружается");
        wifi_up_st = 3;
        wifi_up_t = millis();
      } else {
        wifi_up_fail("500 Internal Server Error", "Прошивка не принята");
      }
    }
  }
}

// ui: web_ui
void ui_esc(String &o, const char *s) {
  for (; *s; s++) {
    if (*s == '"' || *s == '\\') o += '\\';
    if ((uint8_t)*s >= 32) o += *s;
  }
}
// The page itself is public; everything it reads or writes needs the password (argument k).
bool ui_allowed(FLProgNetworkHttpServer *server) {
  const char *pw = "1234";
  if (!pw[0] || server->argumentValueAtKey("k") == pw) return true;
  server->send(403, "text/plain", "auth");
  return false;
}
void ui_page(FLProgNetworkHttpServer *server) {
  // The page is far bigger than the response buffer: stream it straight from flash.
  if (!server->sendStreamProgmem(200, "text/html; charset=utf-8", ui_html, strlen(ui_html))) {
    // The legacy server mode cannot stream from flash: write the page to the client directly (the 200 header is added).
    server->write((const uint8_t *)ui_html, strlen(ui_html));
  }
}
void ui_layout(FLProgNetworkHttpServer *server) {
  if (!ui_allowed(server)) return;
  String o = "{\"t\":\"";
  ui_esc(o, "Панель ESP32");
  o += "\",\"p\":\"";
  ui_esc(o, "Главная");
  o += "\",\"bg\":\"";
  ui_esc(o, "#101820");
  o += "\",\"y\":";
  o += 1;
  o += ",\"i\":[";
  for (uint8_t i = 0; i < flpUiCount; i++) {
    FlpUiItem *it = flpUiItems[i];
    if (i) o += ',';
    o += "{\"k\":\""; o += (it->kind + 4);
    o += "\",\"l\":\""; ui_esc(o, it->caption);
    o += "\",\"u\":\""; ui_esc(o, it->unit);
    o += "\",\"c\":"; o += it->col;
    o += ",\"r\":"; o += it->row;
    o += ",\"w\":"; o += it->w;
    o += ",\"h\":"; o += it->h;
    o += ",\"g\":"; o += it->page;
    o += ",\"d\":"; o += it->dec;
    o += ",\"a\":"; o += String(it->lo, 3);
    o += ",\"b\":"; o += String(it->hi, 3);
    o += ",\"s\":"; o += String(it->step, 3);
    o += ",\"o\":\""; if (it->color) ui_esc(o, it->color);
    o += "\",\"q\":\""; if (it->back) ui_esc(o, it->back);
    o += '"';
    o += '}';
  }
  o += "]}";
  server->send(200, "application/json", o);
}
void ui_state(FLProgNetworkHttpServer *server) {
  if (!ui_allowed(server)) return;
  String o = "[";
  for (uint8_t i = 0; i < flpUiCount; i++) {
    FlpUiItem *it = flpUiItems[i];
    if (i) o += ',';
    if (!strcmp(it->kind, "web_text") || !strcmp(it->kind, "web_net")) {
      o += '"'; ui_esc(o, it->text.c_str()); o += '"';
    } else {
      o += String(it->value, (unsigned int)it->dec);
    }
  }
  o += ']';
  server->send(200, "application/json", o);
}
void ui_set(FLProgNetworkHttpServer *server) {
  if (!ui_allowed(server)) return;
  if (server->hasArgumentKey("i") && server->hasArgumentKey("v")) {
    int i = server->argumentValueAtKey("i").toInt();
    float v = server->argumentValueAtKey("v").toFloat();
    if (i >= 0 && i < flpUiCount && flpUiItems[i]->rw) {
      FlpUiItem *it = flpUiItems[i];
      if (it->hi > it->lo) v = constrain(v, it->lo, it->hi);
      it->value = v;
    }
  }
  server->send(200, "text/plain", "ok");
}
#ifdef FLP_WEB_NET
// Network settings form: the widget block validates and saves what is stored here.
void ui_net(FLProgNetworkHttpServer *server) {
  if (!ui_allowed(server)) return;
  int i = server->argumentValueAtKey("i").toInt();
  if (i >= 0 && i < flpUiCount && !strcmp(flpUiItems[i]->kind, "web_net")) {
    String a[10] = {"s", "p", "d", "i", "g", "m", "a", "b", "o", "w"};
    String t;
    for (uint8_t k = 0; k < 10; k++) {
      String v = server->argumentValueAtKey(a[k]);
      v.replace("\n", " ");
      v.replace("\r", " ");
      if (k) t += '\n';
      t += v;
    }
    flpUiItems[i]->text = t;
    flpUiItems[i]->value = 1;
  }
  server->send(200, "text/plain", "ok");
}
// Wi-Fi scan: the first call starts it, "?" means still running, then one "ssid<TAB>rssi" per line.
void ui_scan(FLProgNetworkHttpServer *server) {
  if (!ui_allowed(server)) return;
  String o;
  int n = WiFi.scanComplete();
  if (n == WIFI_SCAN_FAILED) {
    if (WiFi.getMode() == WIFI_MODE_NULL) WiFi.mode(WIFI_STA);
    else if (WiFi.getMode() == WIFI_MODE_AP) WiFi.mode(WIFI_MODE_APSTA);
    WiFi.scanNetworks(true);
    o = "?";
  } else if (n == WIFI_SCAN_RUNNING) {
    o = "?";
  } else {
    for (int k = 0; k < n; k++) {
      String s = WiFi.SSID(k);
      if (!s.length() || o.startsWith(s + "\t") || o.indexOf("\n" + s + "\t") >= 0) continue;
      String line = s + "\t" + String(WiFi.RSSI(k)) + "\n";
      // The server answers with at most 2 KB: the strongest networks (the scan is sorted by signal), each name once.
      if (o.length() + line.length() > 1800) break;
      o += line;
    }
    WiFi.scanDelete();
  }
  server->send(200, "text/plain", o);
}
#endif
// Chart history, oldest point first.
void ui_hist(FLProgNetworkHttpServer *server) {
  if (!ui_allowed(server)) return;
  int i = server->argumentValueAtKey("i").toInt();
  String o = "[";
  if (i >= 0 && i < flpUiCount && flpUiItems[i]->hist) {
    FlpUiItem *it = flpUiItems[i];
    uint16_t start = (it->hpos + it->hn - it->hfill) % it->hn;
    for (uint16_t k = 0; k < it->hfill; k++) {
      if (k) o += ',';
      o += String(it->hist[(start + k) % it->hn], 2);
    }
  }
  o += ']';
  server->send(200, "application/json", o);
}
// Technical data of the device for the «Система» page.
void ui_info(FLProgNetworkHttpServer *server) {
  if (!ui_allowed(server)) return;
  char mac[18];
  uint8_t *m = flpWifi.mac();
  snprintf(mac, sizeof(mac), "%02X:%02X:%02X:%02X:%02X:%02X", m[0], m[1], m[2], m[3], m[4], m[5]);
  uint32_t ip = flpWifi.clientIsReady() ? (uint32_t)flpWifi.localIP() : (uint32_t)flpWifi.accessPointIP();
  String o = "{\"ip\":\"";
  o += IPAddress(ip).toString();
  o += "\",\"mac\":\""; o += mac;
  o += "\",\"rssi\":"; o += String(flpWifi.rssi());
  o += ",\"up\":"; o += String(millis() / 1000);
  o += ",\"chip\":\""; o += ESP.getChipModel();
  o += "\",\"cpu\":"; o += String(ESP.getCpuFreqMHz());
  o += ",\"heap\":"; o += String(ESP.getFreeHeap());
  o += ",\"heapAll\":"; o += String(ESP.getHeapSize());
  o += ",\"flash\":"; o += String(ESP.getFlashChipSize());
  o += ",\"sketch\":"; o += String(ESP.getSketchSize());
  o += ",\"free\":"; o += String(ESP.getFreeSketchSpace());
  o += ",\"build\":\"" __DATE__ " " __TIME__ "\"";
#ifdef FLP_WEB_UPD
  o += ",\"u\":true";
#else
  o += ",\"u\":false";
#endif
  o += '}';
  server->send(200, "application/json", o);
}

// net: web_net
// The page sends the whole form as ten lines (ssid, password, dhcp, ip, gateway, subnet, ap ssid, ap password, mode, ap delay); an empty password keeps the saved one.
String net_line(const String &s, int n) {
  int from = 0;
  for (int k = 0; k < n; k++) {
    int p = s.indexOf('\n', from);
    if (p < 0) return String();
    from = p + 1;
  }
  int e = s.indexOf('\n', from);
  return e < 0 ? s.substring(from) : s.substring(from, e);
}
void net_show() {
  net_ui.text = flpNet.f[0] + "\n\n" + flpNet.f[2] + "\n" + flpNet.f[3] + "\n" + flpNet.f[4] + "\n" + flpNet.f[5] + "\n" + flpNet.f[6] + "\n\n" + flpNet.f[8] + "\n" + flpNet.f[9];
}
static bool net_pwok(const String &p) {
  if (p.length() == 0) return true;
  for (size_t k = 0; k < p.length(); k++) if (p[k] < 32 || p[k] > 126) return false;
  if (p.length() >= 8 && p.length() <= 63) return true;
  if (p.length() != 64) return false;
  for (size_t k = 0; k < 64; k++) if (!isxdigit(p[k])) return false;
  return true;
}
static bool net_ssidok(const String &s) {
  if (s.length() > 32) return false;
  for (size_t k = 0; k < s.length(); k++) if ((uint8_t)s[k] < 32) return false;
  return true;
}

void setup() {
  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);
  txt_out.reserve(32);
  net_ssid.reserve(32);
  net_password.reserve(32);
  net_ip.reserve(32);
  net_gateway.reserve(32);
  net_subnet.reserve(32);

  // wifi: wifi
  wifi_ota.setHostname("wifi");

  // ui: web_ui
  flpWebSrv.addHandler("/", ui_page);
  flpWebSrv.addHandler("/ui/layout", ui_layout);
  flpWebSrv.addHandler("/ui/state", ui_state);
  flpWebSrv.addHandler("/ui/set", ui_set);
  flpWebSrv.addHandler("/ui/hist", ui_hist);
  #ifdef FLP_WEB_NET
  flpWebSrv.addHandler("/ui/net", ui_net);
  flpWebSrv.addHandler("/ui/scan", ui_scan);
  #endif
  flpWebSrv.addHandler("/ui/info", ui_info);

  // head: web_label
  flpUiAdd(&head_ui);
  head_ui.color = "";
  head_ui.back = "";

  // sw: web_switch
  flpUiAdd(&sw_ui);
  sw_ui.color = "#f5a623";
  sw_ui.back = "";

  // btn: web_button
  flpUiAdd(&btn_ui);
  btn_ui.color = "";
  btn_ui.back = "";

  // sld: web_slider
  flpUiAdd(&sld_ui);
  sld_ui.color = "";
  sld_ui.back = "";

  // inp: web_input
  flpUiAdd(&inp_ui);
  inp_ui.color = "";
  inp_ui.back = "";

  // val: web_value
  flpUiAdd(&val_ui);
  val_ui.color = "";
  val_ui.back = "";

  // bar: web_gauge
  flpUiAdd(&bar_ui);
  bar_ui.color = "";
  bar_ui.back = "";

  // lamp: web_led
  flpUiAdd(&lamp_ui);
  lamp_ui.color = "";
  lamp_ui.back = "";

  // led: digital_out
  pinMode(8, OUTPUT);

  // info: web_text
  flpUiAdd(&info_ui);
  info_ui.color = "";
  info_ui.back = "";

  // line: web_shape
  flpUiAdd(&line_ui);
  line_ui.color = "#4a90d9";
  line_ui.back = "";

  // box: web_shape
  flpUiAdd(&box_ui);
  box_ui.color = "#e94f37";
  box_ui.back = "";

  // trend: web_chart
  flpUiAdd(&trend_ui);
  trend_ui.color = "#7ed321";
  trend_ui.back = "";

  // net: web_net
  net_prefs.begin("flp", false);
  // Defaults come from the editor: when they change (a new flash with other settings), the saved page values are dropped.
  {
    String init[10] = { "MyNetwork", "", "1", "192.168.1.177", "192.168.1.1", "255.255.255.0", "FLP_ESP32", "", "0", "30"};
    uint32_t sig = 2166136261UL;
    for (int k = 0; k < 10; k++) {
      for (size_t n = 0; n < init[k].length(); n++) sig = (sig ^ (uint8_t)init[k][n]) * 16777619UL;
      sig = (sig ^ 0xFF) * 16777619UL;
    }
    if (net_prefs.getUInt("net_h", 0) != sig) {
      static const char *keys[10] = {"_s", "_p", "_d", "_i", "_g", "_m", "_a", "_b", "_o", "_w"};
      for (int k = 0; k < 10; k++) net_prefs.remove((String("net") + keys[k]).c_str());
      net_prefs.putUInt("net_h", sig);
    }
  }
  flpNet.f[0] = net_prefs.getString("net_s", "MyNetwork");
  flpNet.f[1] = net_prefs.getString("net_p", "");
  flpNet.f[2] = net_prefs.getString("net_d", "1");
  flpNet.f[3] = net_prefs.getString("net_i", "192.168.1.177");
  flpNet.f[4] = net_prefs.getString("net_g", "192.168.1.1");
  flpNet.f[5] = net_prefs.getString("net_m", "255.255.255.0");
  flpNet.f[6] = net_prefs.getString("net_a", "FLP_ESP32");
  flpNet.f[7] = net_prefs.getString("net_b", "");
  flpNet.f[8] = net_prefs.getString("net_o", "0");
  flpNet.f[9] = net_prefs.getString("net_w", "30");
  net_show();
  flpUiAdd(&net_ui);
  net_ui.color = "";
  net_ui.back = "";

  // tank: web_tank
  flpUiAdd(&tank_ui);
  tank_ui.color = "#4a90d9";
  tank_ui.back = "";

  // clock: web_clock
  flpUiAdd(&clock_ui);
  clock_ui.color = "";
  clock_ui.back = "";

  // pic: web_image
  flpUiAdd(&pic_ui);
  pic_ui.color = "";
  pic_ui.back = "";

  // fan: web_icon
  flpUiAdd(&fan_ui);
  fan_ui.color = "#2da44e";
  fan_ui.back = "";

  // arrow: web_shape
  flpUiAdd(&arrow_ui);
  arrow_ui.color = "#4a90d9";
  arrow_ui.back = "";

  // panel: web_shape
  flpUiAdd(&panel_ui);
  panel_ui.color = "";
  panel_ui.back = "";

  // sepv: web_shape
  flpUiAdd(&sepv_ui);
  sepv_ui.color = "";
  sepv_ui.back = "";
}

void loop() {
  // wifi: wifi
  {
    // Settings written while running are applied when they change.
    String cfg = String(flpNet.f[0].c_str()) + "|" + flpNet.f[1].c_str() + "|" + (((flpNet.f[2] != "0")) ? "1" : "0") + "|" + flpNet.f[3].c_str() + "|" + flpNet.f[4].c_str() + "|" + flpNet.f[5].c_str() + "|" + flpNet.f[6].c_str() + "|" + flpNet.f[7].c_str();
    if (cfg != wifi_cfg) {
      bool first = wifi_cfg.length() == 0;
      wifi_cfg = cfg;
      flpWifi.setClientSsid(flpNet.f[0].c_str());
      flpWifi.setClientPassword(flpNet.f[1].c_str());
      if ((flpNet.f[2] != "0")) {
        flpWifi.setDhcp();
      } else {
        IPAddress addr;
        addr.fromString(flpNet.f[3].c_str());
        flpWifi.localIP(addr);
        addr.fromString(flpNet.f[4].c_str());
        flpWifi.gateway(addr);
        flpWifi.dns(addr);
        addr.fromString(flpNet.f[5].c_str());
        flpWifi.subnet(addr);
        flpWifi.resetDhcp();
      }
      flpWifi.setApSsid(flpNet.f[6].c_str());
      flpWifi.setApPassword(flpNet.f[7].c_str());
      if (!first) {
        flpWifi.clientOff();
        flpWifi.apOff();
        wifi_state = 255;
      }
    }
  }
  {
    // Client and access point follow the inputs; switched only when they change.
    int32_t mode = flpNet.eff;
    if (mode < 0 || mode > 2) {
      mode = 0;
    }
    uint8_t state = (uint8_t)(mode + 1);
    if (state != wifi_state) {
      wifi_state = state;
      if (state & 1) {
        flpWifi.clientOn();
      } else {
        flpWifi.clientOff();
      }
      if (state & 2) {
        flpWifi.apOn();
      } else {
        flpWifi.apOff();
      }
    }
  }
  flpWifi.pool();
  wifi_ready = flpWifi.clientIsReady();
  {
    uint32_t ip = 0;
    if (flpWifi.clientIsReady()) {
      ip = (uint32_t)flpWifi.localIP();
    } else if (flpWifi.apIsReady()) {
      ip = (uint32_t)flpWifi.accessPointIP();
    }
    if (ip != wifi_ip_raw) {
      wifi_ip_raw = ip;
      wifi_address = ip ? IPAddress(ip).toString() : String();
    }
  }
  if (true) {
    wifi_web.pool();
  }
  if (true && true) {
    wifi_up_poll();
  }
  if (true) {
    if (!wifi_ota_on) {
      wifi_ota_on = true;
      if (""[0] != 0) {
        wifi_ota.setPasswordValue("");
      }
      wifi_ota.begin(flpWifi);
    }
    wifi_ota.pool();
  }

  // sw: web_switch
  sw_out = sw_ui.value != 0;

  // btn: web_button
  btn_out = btn_ui.value != 0;

  // sld: web_slider
  sld_out = sld_ui.value;

  // inp: web_input
  inp_out = inp_ui.value;

  // val: web_value
  val_ui.value = sld_out;

  // bar: web_gauge
  bar_ui.value = sld_out;

  // any: or
  any_out = sw_out || btn_out;

  // lamp: web_led
  lamp_ui.value = any_out ? 1 : 0;

  // led: digital_out
  digitalWrite(8, (any_out) ? HIGH : LOW);

  // txt: num_to_text
  txt_out = "Уставка: ";
  txt_out += String(inp_out, 1);
  txt_out += "";

  // info: web_text
  info_ui.text = txt_out;

  // trend: web_chart
  trend_ui.value = sld_out;
  flpUiSample(&trend_ui);

  // net: web_net
  net_changed = false;
  if (net_ui.value != 0) {
    net_ui.value = 0;
    String t = net_ui.text;
    String pw = net_line(t, 1);
    String apw = net_line(t, 7);
    String nf[10] = { net_line(t, 0), pw.length() ? pw : flpNet.f[1], net_line(t, 2), net_line(t, 3), net_line(t, 4), net_line(t, 5), net_line(t, 6), apw.length() ? apw : flpNet.f[7], net_line(t, 8), net_line(t, 9)};
    bool valid = net_pwok(pw) && net_pwok(apw) && net_ssidok(nf[0]) && net_ssidok(nf[6]);
    static const char *keys[10] = {"_s", "_p", "_d", "_i", "_g", "_m", "_a", "_b", "_o", "_w"};
    for (int k = 0; k < 10 && valid; k++) {
      if (nf[k] != flpNet.f[k]) {
        flpNet.f[k] = nf[k];
        // Flash wears out: only changed fields are written.
        net_prefs.putString((String("net") + keys[k]).c_str(), nf[k]);
        net_changed = true;
      }
    }
    net_show();
  }
  {
    // Automatic mode: client only, the access point joins when there is no connection for the delay.
    static uint32_t since = 0;
    static bool apUp = false;
    static uint32_t joined = 0;
    int m = flpNet.f[8].toInt();
    if (m == 3) {
      if (WiFi.status() == WL_CONNECTED) {
        // The access point stays up for 10 s after the client got connected, then goes off.
        if (!joined) joined = millis();
        if (millis() - joined >= 10000UL) apUp = false;
        since = millis();
      } else {
        joined = 0;
        if (!apUp && millis() - since >= (uint32_t)flpNet.f[9].toInt() * 1000UL) {
          apUp = true;
        }
      }
      m = apUp ? 2 : 0;
    }
    flpNet.eff = m;
  }
  net_ssid = flpNet.f[0];
  net_password = flpNet.f[1];
  net_dhcp = flpNet.f[2] != "0";
  net_ip = flpNet.f[3];
  net_gateway = flpNet.f[4];
  net_subnet = flpNet.f[5];
  net_mode = flpNet.f[8].toInt();

  // tank: web_tank
  tank_ui.value = sld_out;

  // fan: web_icon
  fan_ui.value = sw_out ? 1 : 0;
}

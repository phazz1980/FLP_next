//! Firmware code of the web interface blocks: the page server `web_ui`, the network
//! settings form `web_net` and the widgets. Big fixed texts (the page, its handlers) are
//! kept as constants; `@NAME@` marks where the block's own values go (see `fill`).

use crate::cpp::{CppCtx, CppParts, Lines, Raw};

/// Registry of widget items, shared by all web blocks.
const UI_REGISTRY: &str = r##"// Web interface registry: widget blocks register their items, the web_ui block serves them.
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
#define FLP_WEBUI 1"##;

/// The page served at `/`.
const UI_PAGE: &str = r##"const char @ID@_html[] PROGMEM = R"FLPUI(<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>FLP_next</title>
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
</script></body></html>)FLPUI";"##;

/// Request handlers of the page server.
const UI_HANDLERS: &str = r##"void @ID@_esc(String &o, const char *s) {
  for (; *s; s++) {
    if (*s == '"' || *s == '\\') o += '\\';
    if ((uint8_t)*s >= 32) o += *s;
  }
}
// The page itself is public; everything it reads or writes needs the password (argument k).
bool @ID@_allowed(FLProgNetworkHttpServer *server) {
  const char *pw = @P:password@;
  if (!pw[0] || server->argumentValueAtKey("k") == pw) return true;
  server->send(403, "text/plain", "auth");
  return false;
}
void @ID@_page(FLProgNetworkHttpServer *server) {
  // The page is far bigger than the response buffer: stream it straight from flash.
  if (!server->sendStreamProgmem(200, "text/html; charset=utf-8", @ID@_html, strlen(@ID@_html))) {
    // The legacy server mode cannot stream from flash: write the page to the client directly (the 200 header is added).
    server->write((const uint8_t *)@ID@_html, strlen(@ID@_html));
  }
}
void @ID@_layout(FLProgNetworkHttpServer *server) {
  if (!@ID@_allowed(server)) return;
  String o = "{\"t\":\"";
  @ID@_esc(o, @P:title@);
  o += "\",\"p\":\"";
  @ID@_esc(o, @P:pages@);
  o += "\",\"bg\":\"";
  @ID@_esc(o, @P:bg@);
  o += "\",\"y\":";
  o += @SYSTEM@;
  o += ",\"i\":[";
  for (uint8_t i = 0; i < flpUiCount; i++) {
    FlpUiItem *it = flpUiItems[i];
    if (i) o += ',';
    o += "{\"k\":\""; o += (it->kind + 4);
    o += "\",\"l\":\""; @ID@_esc(o, it->caption);
    o += "\",\"u\":\""; @ID@_esc(o, it->unit);
    o += "\",\"c\":"; o += it->col;
    o += ",\"r\":"; o += it->row;
    o += ",\"w\":"; o += it->w;
    o += ",\"h\":"; o += it->h;
    o += ",\"g\":"; o += it->page;
    o += ",\"d\":"; o += it->dec;
    o += ",\"a\":"; o += String(it->lo, 3);
    o += ",\"b\":"; o += String(it->hi, 3);
    o += ",\"s\":"; o += String(it->step, 3);
    o += ",\"o\":\""; if (it->color) @ID@_esc(o, it->color);
    o += "\",\"q\":\""; if (it->back) @ID@_esc(o, it->back);
    o += '"';
    o += '}';
  }
  o += "]}";
  server->send(200, "application/json", o);
}
void @ID@_state(FLProgNetworkHttpServer *server) {
  if (!@ID@_allowed(server)) return;
  String o = "[";
  for (uint8_t i = 0; i < flpUiCount; i++) {
    FlpUiItem *it = flpUiItems[i];
    if (i) o += ',';
    if (!strcmp(it->kind, "web_text") || !strcmp(it->kind, "web_net")) {
      o += '"'; @ID@_esc(o, it->text.c_str()); o += '"';
    } else {
      o += String(it->value, (unsigned int)it->dec);
    }
  }
  o += ']';
  server->send(200, "application/json", o);
}
void @ID@_set(FLProgNetworkHttpServer *server) {
  if (!@ID@_allowed(server)) return;
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
void @ID@_net(FLProgNetworkHttpServer *server) {
  if (!@ID@_allowed(server)) return;
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
void @ID@_scan(FLProgNetworkHttpServer *server) {
  if (!@ID@_allowed(server)) return;
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
void @ID@_hist(FLProgNetworkHttpServer *server) {
  if (!@ID@_allowed(server)) return;
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
}"##;

/// Handler of the system page (`system = true`).
const UI_INFO: &str = r##"// Technical data of the device for the «Система» page.
void @ID@_info(FLProgNetworkHttpServer *server) {
  if (!@ID@_allowed(server)) return;
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
}"##;

/// Handlers registered on the shared web server.
const UI_SETUP: &str = r##"flpWebSrv.addHandler("/", @ID@_page);
flpWebSrv.addHandler("/ui/layout", @ID@_layout);
flpWebSrv.addHandler("/ui/state", @ID@_state);
flpWebSrv.addHandler("/ui/set", @ID@_set);
flpWebSrv.addHandler("/ui/hist", @ID@_hist);
#ifdef FLP_WEB_NET
flpWebSrv.addHandler("/ui/net", @ID@_net);
flpWebSrv.addHandler("/ui/scan", @ID@_scan);
#endif"##;

/// Tells the page server to add the network form handlers.
const NET_DEFINES: &str = r##"// Tells the web page block to add the network form handlers (/ui/net, /ui/scan).
#define FLP_WEB_NET"##;

/// Network settings shared with the Wi-Fi and Ethernet blocks.
const NET_SHARED: &str = r##"// Network settings shared with the Wi-Fi and Ethernet blocks: ssid, password, dhcp, ip, gateway, subnet, ap ssid, ap password, mode, ap delay.
struct FlpNet {
  String f[10];
  // Mode actually applied: differs from f[8] in the automatic access point mode.
  int eff = 0;
};
FlpNet flpNet;"##;

/// Item and storage of the network form.
const NET_GLOBAL: &str = r##"FlpUiItem @ID@_ui = {"web_net", @P:caption@, "", @V:col@, @V:row@, @V:w@, @V:h@, @V:page@, true, 0, 0, 0, 0, 0};
Preferences @ID@_prefs;"##;

/// Form parsing and validation.
const NET_FUNCTIONS: &str = r##"// The page sends the whole form as ten lines (ssid, password, dhcp, ip, gateway, subnet, ap ssid, ap password, mode, ap delay); an empty password keeps the saved one.
String @ID@_line(const String &s, int n) {
  int from = 0;
  for (int k = 0; k < n; k++) {
    int p = s.indexOf('\n', from);
    if (p < 0) return String();
    from = p + 1;
  }
  int e = s.indexOf('\n', from);
  return e < 0 ? s.substring(from) : s.substring(from, e);
}
void @ID@_show() {
  @ID@_ui.text = flpNet.f[0] + "\n\n" + flpNet.f[2] + "\n" + flpNet.f[3] + "\n" + flpNet.f[4] + "\n" + flpNet.f[5] + "\n" + flpNet.f[6] + "\n\n" + flpNet.f[8] + "\n" + flpNet.f[9];
}
static bool @ID@_pwok(const String &p) {
  if (p.length() == 0) return true;
  for (size_t k = 0; k < p.length(); k++) if (p[k] < 32 || p[k] > 126) return false;
  if (p.length() >= 8 && p.length() <= 63) return true;
  if (p.length() != 64) return false;
  for (size_t k = 0; k < 64; k++) if (!isxdigit(p[k])) return false;
  return true;
}
static bool @ID@_ssidok(const String &s) {
  if (s.length() > 32) return false;
  for (size_t k = 0; k < s.length(); k++) if ((uint8_t)s[k] < 32) return false;
  return true;
}"##;

/// Restore the saved settings.
const NET_SETUP: &str = r##"@ID@_prefs.begin("flp", false);
// Defaults come from the editor: when they change (a new flash with other settings), the saved page values are dropped.
{
  String init[10] = { @P:init_ssid@, @P:init_password@, "@DHCP@", @P:init_ip@, @P:init_gateway@, @P:init_subnet@, @P:init_ap_ssid@, @P:init_ap_password@, "@V:init_mode@", "@V:init_ap_delay@"};
  uint32_t sig = 2166136261UL;
  for (int k = 0; k < 10; k++) {
    for (size_t n = 0; n < init[k].length(); n++) sig = (sig ^ (uint8_t)init[k][n]) * 16777619UL;
    sig = (sig ^ 0xFF) * 16777619UL;
  }
  if (@ID@_prefs.getUInt("@ID12@_h", 0) != sig) {
    static const char *keys[10] = {"_s", "_p", "_d", "_i", "_g", "_m", "_a", "_b", "_o", "_w"};
    for (int k = 0; k < 10; k++) @ID@_prefs.remove((String("@ID12@") + keys[k]).c_str());
    @ID@_prefs.putUInt("@ID12@_h", sig);
  }
}
flpNet.f[0] = @ID@_prefs.getString("@ID12@_s", @P:init_ssid@);
flpNet.f[1] = @ID@_prefs.getString("@ID12@_p", @P:init_password@);
flpNet.f[2] = @ID@_prefs.getString("@ID12@_d", "@DHCP@");
flpNet.f[3] = @ID@_prefs.getString("@ID12@_i", @P:init_ip@);
flpNet.f[4] = @ID@_prefs.getString("@ID12@_g", @P:init_gateway@);
flpNet.f[5] = @ID@_prefs.getString("@ID12@_m", @P:init_subnet@);
flpNet.f[6] = @ID@_prefs.getString("@ID12@_a", @P:init_ap_ssid@);
flpNet.f[7] = @ID@_prefs.getString("@ID12@_b", @P:init_ap_password@);
flpNet.f[8] = @ID@_prefs.getString("@ID12@_o", "@V:init_mode@");
flpNet.f[9] = @ID@_prefs.getString("@ID12@_w", "@V:init_ap_delay@");
@ID@_show();
flpUiAdd(&@ID@_ui);
@ID@_ui.color = @P:color@;
@ID@_ui.back = @P:back@;"##;

/// Apply the form and publish the settings.
const NET_LOOP: &str = r##"@O:changed@ = false;
if (@ID@_ui.value != 0) {
  @ID@_ui.value = 0;
  String t = @ID@_ui.text;
  String pw = @ID@_line(t, 1);
  String apw = @ID@_line(t, 7);
  String nf[10] = { @ID@_line(t, 0), pw.length() ? pw : flpNet.f[1], @ID@_line(t, 2), @ID@_line(t, 3), @ID@_line(t, 4), @ID@_line(t, 5), @ID@_line(t, 6), apw.length() ? apw : flpNet.f[7], @ID@_line(t, 8), @ID@_line(t, 9)};
  bool valid = @ID@_pwok(pw) && @ID@_pwok(apw) && @ID@_ssidok(nf[0]) && @ID@_ssidok(nf[6]);
  static const char *keys[10] = {"_s", "_p", "_d", "_i", "_g", "_m", "_a", "_b", "_o", "_w"};
  for (int k = 0; k < 10 && valid; k++) {
    if (nf[k] != flpNet.f[k]) {
      flpNet.f[k] = nf[k];
      // Flash wears out: only changed fields are written.
      @ID@_prefs.putString((String("@ID12@") + keys[k]).c_str(), nf[k]);
      @O:changed@ = true;
    }
  }
  @ID@_show();
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
@O:ssid@ = flpNet.f[0];
@O:password@ = flpNet.f[1];
@O:dhcp@ = flpNet.f[2] != "0";
@O:ip@ = flpNet.f[3];
@O:gateway@ = flpNet.f[4];
@O:subnet@ = flpNet.f[5];
@O:mode@ = flpNet.f[8].toInt();"##;

/// A value of the block as the text a template would print.
fn value_text(e: &CppCtx, name: &str) -> String {
    match e.value.get(name) {
        Some(Raw::Int(v)) => v.to_string(),
        Some(Raw::Float(v)) => v.to_string(),
        Some(Raw::Text(v)) => v.clone(),
        Some(Raw::Bool(v)) => v.to_string(),
        None => String::new(),
    }
}

/// The page the device serves at `/` (the raw string of the page constant), so the preview
/// in the constructor is always the page that goes into the firmware.
pub fn device_page() -> &'static str {
    const OPEN: &str = "R\"FLPUI(";
    const CLOSE: &str = ")FLPUI\"";
    let start = UI_PAGE.find(OPEN).map_or(0, |i| i + OPEN.len());
    let len = UI_PAGE[start..]
        .find(CLOSE)
        .unwrap_or(UI_PAGE.len() - start);
    &UI_PAGE[start..start + len]
}

/// Puts the block's values into a constant text: `@ID@` (block id), `@ID12@` (its first
/// twelve characters), `@P:name@` (parameter), `@V:name@` (plain value), `@O:name@`
/// (output variable) and the ready-made `@SYSTEM@` and `@DHCP@` flags. Other `@` stay as is.
fn fill(e: &CppCtx, text: &str) -> String {
    let id12: String = e.id.chars().take(12).collect();
    let flag = |n: &str| if e.flag(n) { "1" } else { "0" };
    let mut out = String::with_capacity(text.len() + 256);
    let mut rest = text;
    while let Some(at) = rest.find('@') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let named = |kind: &str| {
            let tail = rest.strip_prefix(kind)?;
            let end = tail.find('@')?;
            let name = &tail[..end];
            (!name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
                .then_some((name, kind.len() + end + 1))
        };
        if let Some(r) = rest.strip_prefix("@ID@") {
            out.push_str(&e.id);
            rest = r;
        } else if let Some(r) = rest.strip_prefix("@ID12@") {
            out.push_str(&id12);
            rest = r;
        } else if let Some(r) = rest.strip_prefix("@SYSTEM@") {
            out.push_str(flag("system"));
            rest = r;
        } else if let Some(r) = rest.strip_prefix("@DHCP@") {
            out.push_str(flag("init_dhcp"));
            rest = r;
        } else if let Some((n, len)) = named("@P:") {
            out.push_str(e.param(n));
            rest = &rest[len..];
        } else if let Some((n, len)) = named("@V:") {
            out.push_str(&value_text(e, n));
            rest = &rest[len..];
        } else if let Some((n, len)) = named("@O:") {
            out.push_str(e.output(n));
            rest = &rest[len..];
        } else {
            out.push('@');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

pub(crate) fn web_ui(e: &CppCtx) -> CppParts {
    let mut handlers = fill(e, UI_HANDLERS);
    let mut setup = fill(e, UI_SETUP);
    if e.flag("system") {
        handlers.push('\n');
        handlers.push_str(&fill(e, UI_INFO));
        setup.push_str(&fill(e, "\nflpWebSrv.addHandler(\"/ui/info\", @ID@_info);"));
    }
    CppParts {
        once_global: UI_REGISTRY.to_owned(),
        global: fill(e, UI_PAGE),
        functions: handlers,
        setup,
        ..Default::default()
    }
}

pub(crate) fn web_net(e: &CppCtx) -> CppParts {
    CppParts {
        defines: NET_DEFINES.to_owned(),
        once_global: NET_SHARED.to_owned(),
        global: fill(e, NET_GLOBAL),
        functions: fill(e, NET_FUNCTIONS),
        setup: fill(e, NET_SETUP),
        looped: fill(e, NET_LOOP),
        ..Default::default()
    }
}

/// The item every widget registers; `tail` is the part after the page number:
/// `rw, init, lo, hi, step, dec`.
fn item(e: &CppCtx, kind: &str, unit: &str, tail: &str) -> String {
    format!(
        "FlpUiItem {}_ui = {{\"web_{kind}\", {}, {unit}, {}, {}, {}, {}, {}, {tail}}};",
        e.id,
        e.param("caption"),
        value_text(e, "col"),
        value_text(e, "row"),
        value_text(e, "w"),
        value_text(e, "h"),
        value_text(e, "page"),
    )
}

/// The registration and colours are the same for all widgets.
fn register(e: &CppCtx, comment: &str, global_item: String, looped: String) -> CppParts {
    let id = &e.id;
    let mut g = Lines::default();
    if !comment.is_empty() {
        g.line(comment);
    }
    g.line(global_item);
    CppParts {
        global: g.finish(),
        setup: format!(
            "flpUiAdd(&{id}_ui);\n{id}_ui.color = {};\n{id}_ui.back = {};",
            e.param("color"),
            e.param("back")
        ),
        looped,
        ..Default::default()
    }
}

fn plain(e: &CppCtx, kind: &str, tail: &str, looped: String) -> CppParts {
    register(e, "", item(e, kind, "\"\"", tail), looped)
}

pub(crate) fn web_button(e: &CppCtx) -> CppParts {
    plain(
        e,
        "button",
        "true, 0, 0, 0, 0, 0",
        format!("{} = {}_ui.value != 0;", e.output("out"), e.id),
    )
}

pub(crate) fn web_switch(e: &CppCtx) -> CppParts {
    let init = u8::from(e.flag("initial"));
    plain(
        e,
        "switch",
        &format!("true, {init}, 0, 0, 0, 0"),
        format!("{} = {}_ui.value != 0;", e.output("out"), e.id),
    )
}

pub(crate) fn web_label(e: &CppCtx) -> CppParts {
    plain(e, "label", "false, 0, 0, 0, 0, 0", String::new())
}

pub(crate) fn web_clock(e: &CppCtx) -> CppParts {
    let (s, d) = (u8::from(e.flag("seconds")), u8::from(e.flag("date")));
    plain(
        e,
        "clock",
        &format!("false, 0, {s}, {d}, 0, 0"),
        String::new(),
    )
}

pub(crate) fn web_led(e: &CppCtx) -> CppParts {
    plain(
        e,
        "led",
        "false, 0, 0, 0, 0, 0",
        format!("{}_ui.value = {} ? 1 : 0;", e.id, e.input("in")),
    )
}

pub(crate) fn web_text(e: &CppCtx) -> CppParts {
    plain(
        e,
        "text",
        "false, 0, 0, 0, 0, 0",
        format!("{}_ui.text = {};", e.id, e.input("in")),
    )
}

pub(crate) fn web_icon(e: &CppCtx) -> CppParts {
    register(
        e,
        "// The icon number travels in `lo`.",
        item(
            e,
            "icon",
            "\"\"",
            &format!("false, 0, {}, 0, 0, 0", value_text(e, "icon")),
        ),
        format!("{}_ui.value = {} ? 1 : 0;", e.id, e.input("in")),
    )
}

pub(crate) fn web_image(e: &CppCtx) -> CppParts {
    register(
        e,
        "// The image address travels in `unit`; the fit mode in `lo`.",
        item(
            e,
            "image",
            e.param("url"),
            &format!("false, 0, {}, 0, 0, 0", u8::from(e.flag("fit"))),
        ),
        String::new(),
    )
}

pub(crate) fn web_shape(e: &CppCtx) -> CppParts {
    register(
        e,
        "// The shape code travels in `lo` and the thickness in `hi` (a shape has no value).",
        item(
            e,
            "shape",
            "\"\"",
            &format!(
                "false, 0, {}, {}, 0, 0",
                value_text(e, "shape"),
                value_text(e, "thickness")
            ),
        ),
        String::new(),
    )
}

pub(crate) fn web_gauge(e: &CppCtx) -> CppParts {
    register(
        e,
        "",
        item(
            e,
            "gauge",
            e.param("unit"),
            &format!("false, 0, {}, {}, 0, 1", e.param("min"), e.param("max")),
        ),
        format!("{}_ui.value = {};", e.id, e.input("in")),
    )
}

pub(crate) fn web_tank(e: &CppCtx) -> CppParts {
    register(
        e,
        "",
        item(
            e,
            "tank",
            e.param("unit"),
            &format!(
                "false, 0, {}, {}, 0, {}",
                e.param("min"),
                e.param("max"),
                value_text(e, "decimals")
            ),
        ),
        format!("{}_ui.value = {};", e.id, e.input("in")),
    )
}

pub(crate) fn web_value(e: &CppCtx) -> CppParts {
    register(
        e,
        "",
        item(
            e,
            "value",
            e.param("unit"),
            &format!("false, 0, 0, 0, 0, {}", e.param("decimals")),
        ),
        format!("{}_ui.value = {};", e.id, e.input("in")),
    )
}

/// A slider or a number field: the page writes the value.
fn adjustable(e: &CppCtx, kind: &str) -> CppParts {
    register(
        e,
        "",
        item(
            e,
            kind,
            e.param("unit"),
            &format!(
                "true, {}, {}, {}, {}, 3",
                e.param("initial"),
                e.param("min"),
                e.param("max"),
                e.param("step")
            ),
        ),
        format!("{} = {}_ui.value;", e.output("out"), e.id),
    )
}

pub(crate) fn web_slider(e: &CppCtx) -> CppParts {
    adjustable(e, "slider")
}

pub(crate) fn web_input(e: &CppCtx) -> CppParts {
    adjustable(e, "input")
}

pub(crate) fn web_chart(e: &CppCtx) -> CppParts {
    let (id, points) = (&e.id, value_text(e, "points"));
    let mut g = Lines::default();
    g.line(format!("float {id}_buf[{points}];")).line(format!(
        "FlpUiItem {id}_ui = {{\"web_chart\", {}, {}, {}, {}, {}, {}, {}, false, 0, {}, {}, {}, 1, String(), {id}_buf, {points}}};",
        e.param("caption"),
        e.param("unit"),
        value_text(e, "col"),
        value_text(e, "row"),
        value_text(e, "w"),
        value_text(e, "h"),
        value_text(e, "page"),
        e.param("min"),
        e.param("max"),
        value_text(e, "period"),
    ));
    CppParts {
        global: g.finish(),
        setup: format!(
            "flpUiAdd(&{id}_ui);\n{id}_ui.color = {};\n{id}_ui.back = {};",
            e.param("color"),
            e.param("back")
        ),
        looped: format!("{id}_ui.value = {};\nflpUiSample(&{id}_ui);", e.input("in")),
        ..Default::default()
    }
}

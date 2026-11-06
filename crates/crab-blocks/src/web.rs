//! The device page of the web interface. Widgets and pages live in the project's `web.toml`
//! and are served by the master firmware of WASM modules; the plain Arduino C++ sketch has
//! no web code. The page text is kept here so the constructor preview shows exactly the page
//! the master serves.

/// The page served at `/`.
const UI_PAGE: &str = r##"const char @ID@_html[] PROGMEM = R"CRABUI(<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>CrabLogic</title>
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
try{K=localStorage.getItem('crabk')||''}catch(e){}
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
 if(k=='textin'){
  var n=h('input');n.type=d.a>0?'password':'text';n.autocomplete='off';n.spellcheck=false;
  w.appendChild(h('small','',d.l));w.appendChild(n);
  n.onchange=function(){S(i,encodeURIComponent(n.value))};
  return[w,function(v){if(document.activeElement!=n&&(d.a<=0||v!==''))n.value=v}]}
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
   F('/ui/net?n='+i+q).then(function(){ms.textContent='Сохранено и применено';nf.p.value=nf.b.value=''}).catch(function(){ms.textContent='Ошибка'})};
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
 g.appendChild(t);
 function ld(){if(g.hidden)return;F('/ui/info').then(function(r){return r.json()}).then(function(d){
  var s=d.up;R.ip.textContent=d.ip;R.mac.textContent=d.mac;R.rssi.textContent=d.rssi+' дБм';
  R.up.textContent=Math.floor(s/86400)+' д '+Math.floor(s/3600)%24+' ч '+Math.floor(s/60)%60+' мин';
  R.chip.textContent=d.chip+', '+d.cpu+' МГц';R.heap.textContent=Math.round(d.heap/1024)+' из '+Math.round(d.heapAll/1024)+' КБ';
  R.flash.textContent=Math.round(d.flash/1048576)+' МБ';R.app.textContent=Math.round(d.sketch/1024)+' КБ, свободно '+Math.round(d.free/1024)+' КБ';
  R.build.textContent=d.build}).catch(function(){})}
 setInterval(ld,2000);g.onshow=ld;return g}
function poll(){F('/ui/state').then(function(r){return r.json()}).then(function(a){a.forEach(function(v,i){if(E[i])E[i](v)})}).catch(function(){}).then(function(){setTimeout(poll,500)})}
function go(n){P.forEach(function(g,j){g.hidden=j!=n});A.forEach(function(a,j){a.classList.toggle('on',j==n)});history.replaceState(null,'','#'+(n+1));if(P[n].onshow)P[n].onshow()}
function lg(bad){
 M.textContent='';var b=h('div');b.id='lg';b.appendChild(h('h1','','Вход'));
 var p=h('input');p.type='password';p.placeholder='Пароль';var o=h('button','bt','Войти');
 o.onclick=function(){K=p.value;try{localStorage.setItem('crabk',K)}catch(e){}location.reload()};
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
 var B={};d.i.forEach(function(x){if(x.r>0)B[x.g]=Math.max(B[x.g]||0,x.r+x.h-1)});
 d.i.forEach(function(x){if(x.r<1){x.c=1;x.r=(B[x.g]||0)+1;B[x.g]=x.r+x.h-1}});
 d.i.forEach(function(x,i){var m=mk(x,i);P[Math.min(Math.max(x.g,1),nm.length)-1].appendChild(m[0]);E.push(m[1])});
 go(Math.min(Math.max(parseInt(location.hash.slice(1))||1,1),P.length)-1);poll()}).catch(function(){})
</script></body></html>)CRABUI";"##;

/// The page the device serves at `/` (the raw string of the page constant), so the preview
/// in the constructor is always the page that goes into the firmware.
pub fn device_page() -> &'static str {
    const OPEN: &str = "R\"CRABUI(";
    const CLOSE: &str = ")CRABUI\"";
    let start = UI_PAGE.find(OPEN).map_or(0, |i| i + OPEN.len());
    let len = UI_PAGE[start..]
        .find(CLOSE)
        .unwrap_or(UI_PAGE.len() - start);
    &UI_PAGE[start..start + len]
}

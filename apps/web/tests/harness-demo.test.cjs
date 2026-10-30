const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const root = path.resolve(__dirname, '../../..');
const { JSDOM } = require(process.env.JS_DOM_MODULE || path.join(root, 'packages/ui/node_modules/jsdom'));
const script = fs.readFileSync(path.join(root, 'apps/web/dist/docs/navigation.js'), 'utf8');

function boot(t, page = 'design/rsi', reduce = false, focus = false, measure = false) {
  const html = fs.readFileSync(path.join(root, `apps/web/dist/docs/code/9.1.6/${page}/index.html`), 'utf8');
  const dom = new JSDOM(html, { url: `https://aster-ai-lab.github.io/aster/docs/code/9.1.6/${page}/${focus?'?view=graph':''}`, runScripts: 'outside-only', pretendToBeVisual: true });
  t.after(() => dom.window.close());
  const w = dom.window, timers = new Map(), frames = new Map(), animations = [];
  let serial = 0, mediaChange, intersectionChange, clock=0;
  const traces=new Map();
  const media = { matches: reduce, addEventListener(type, callback) { mediaChange = callback; } };
  w.matchMedia = () => media;
  w.IntersectionObserver = function(callback) { intersectionChange=callback; this.observe=()=>{}; };
  w.HTMLCanvasElement.prototype.getContext = function() {
    const arcs=[];traces.set(this,arcs);
    return {setTransform(){},clearRect(){arcs.length=0;},beginPath(){},moveTo(){},lineTo(){},setLineDash(){},stroke(){},arc(...args){arcs.push(args);},fill(){}};
  };
  w.HTMLDialogElement.prototype.showModal=function(){this.open=true;};
  w.HTMLDialogElement.prototype.close=function(){this.open=false;};
  w.Element.prototype.animate = function (keyframes, options) {
    const animation = { keyframes, options, cancelled: false, cancel() { this.cancelled = true; } };
    animations.push(animation); return animation;
  };
  w.setTimeout = callback => { const id = ++serial; timers.set(id, callback); return id; };
  w.clearTimeout = id => timers.delete(id);
  w.requestAnimationFrame = callback => { const id = ++serial; frames.set(id, callback); return id; };
  w.cancelAnimationFrame = id => frames.delete(id);
  const widget = w.document.querySelector('[data-harness-demo]');
  assert.ok(widget, 'the real generated article contains the example');
  if(measure){
    Object.defineProperty(w.HTMLElement.prototype,'offsetWidth',{get(){return this.classList.contains('hd-node')?160:84;}});
    Object.defineProperty(w.HTMLElement.prototype,'offsetHeight',{get(){return 30;}});
    Object.defineProperties(widget.querySelector('.hd-stage'),{clientWidth:{value:620},clientHeight:{value:650}});
  }
  w.eval(script);
  intersectionChange([{isIntersecting:true}]);
  return { w, widget, timers, frames, animations, media, arcs:()=>traces.get(widget.querySelector('canvas')), tetherArcs:()=>traces.get(widget.querySelector('.hd-tether')), intersect(value) { intersectionChange([{isIntersecting:value}]); }, run(duration) { for(let until=clock+duration;clock<until;) { clock+=50; const pending=[...frames.values()]; frames.clear(); pending.forEach(fn=>fn(clock)); } }, advance(time) { clock=time; const pending=[...frames.values()]; frames.clear(); pending.forEach(fn=>fn(time)); }, changeMedia() { mediaChange(); }, click(selector) { const target = widget.querySelector(selector); assert.ok(target, selector); target.click(); return target; } };
}

test('all real cases and Core contracts retain readable records, exact provenance and version isolation without script', () => {
  for(const [page,counts] of [['design/rsi',[9,9,9]],['meta-harness',[9]]]) {
    const dom=new JSDOM(fs.readFileSync(path.join(root,`apps/web/dist/docs/code/9.1.6/${page}/index.html`),'utf8'));
    const widget=dom.window.document.querySelector('[data-harness-demo]'), cases=JSON.parse(widget.dataset.example).cases;
    assert.deepEqual(cases.map(c=>c.nodes.length),counts);
    let records=0;
    for(const item of cases) {
      assert.equal(new Set(item.nodes.map(n=>n.id)).size,item.nodes.length);
      for(const node of item.nodes) {
        assert.ok(node.children.length>0&&node.children.length<=6);
        const grouped=node.groups.flatMap(g=>g.records);assert.equal(grouped.length,node.children.length);assert.equal(new Set(grouped).size,node.children.length);
        const url=new URL(node.source);
        assert.ok(url.hostname==='arxiv.org'||url.pathname.startsWith('/aster-ai-lab/aster/blob/'));
        if(url.hostname==='github.com')assert.ok(fs.existsSync(path.join(root,url.pathname.split('/').slice(5).join('/'))));
        assert.ok(widget.querySelector(`#${widget.id}-${item.id}-node-${node.id}`));
      }
      for(const edge of item.links)for(const id of edge)assert.ok(item.nodes.some(n=>n.id===id));
      for(const step of item.steps){assert.ok(item.nodes.some(n=>n.id===step.focus));if(step.route){assert.equal(step.route.at(-1),step.focus);for(let i=1;i<step.route.length;i++)assert.ok(item.links.some(([a,b])=>a===step.route[i-1]&&b===step.route[i]));}}
      records+=item.steps.length+item.nodes.length;
    }
    assert.equal(widget.querySelectorAll('.hd-transcript section').length,records);
    assert.match(widget.textContent,/非实时运行数据/);
    assert.equal(widget.querySelector('.hd-controls').hidden,true);
    if(cases.length>1){assert.match(widget.textContent,/人工组织/);assert.match(widget.textContent,/没有重新执行/);assert.match(widget.textContent,/文献流程重建/);}
    dom.window.close();
  }
  const old=fs.readFileSync(path.join(root,'apps/web/dist/docs/code/9.1.5/design/rsi/index.html'),'utf8');
  assert.ok(!old.includes('data-harness-demo'));
});

test('a real tool change opens its internal details and reverses to the same node with keyboard focus',t=>{
  const app=boot(t),origin=app.click('[data-node="edit"]');
  assert.equal(origin.getAttribute('aria-expanded'),'true');
  assert.equal(app.widget.querySelector('.hd-lens').hidden,false);
  assert.equal(app.widget.querySelector('.hd-world').inert,true);
  assert.equal(app.widget.querySelectorAll('.hd-lens-nodes button').length,3);
  assert.equal(app.w.document.activeElement,app.widget.querySelector('.hd-close'));
  assert.match(app.animations[0].keyframes[0].transform,/scale\(\.035\)/);
  app.click('.hd-lens-nodes button');
  assert.match(app.widget.querySelector('.hd-detail-body').textContent,/整文件/);
  app.w.document.activeElement.dispatchEvent(new app.w.KeyboardEvent('keydown',{key:'Escape',bubbles:true}));
  assert.match(app.animations.at(-1).keyframes[1].transform,/scale\(\.035\)/);
  app.animations.at(-1).onfinish();
  assert.equal(app.widget.querySelector('.hd-lens').hidden,true);
  assert.equal(app.w.document.activeElement,origin);
  assert.equal(origin.getAttribute('aria-expanded'),'false');
});

test('RSI navigation and return stay in one toolbar outside the scenario and drawing surface',t=>{
  const app=boot(t,'design/rsi',true),toolbar=app.widget.querySelector('.hd-toolbar'),close=toolbar.querySelector('.hd-close');
  assert.ok(toolbar.querySelector('.hd-case-picker'));assert.ok(toolbar.querySelector('.hd-controls'));
  assert.equal(close.hidden,true);assert.equal(toolbar.closest('.hd-stage'),null);
  assert.ok(toolbar.compareDocumentPosition(app.widget.querySelector('.hd-scenario'))&app.w.Node.DOCUMENT_POSITION_FOLLOWING);
  app.click('[data-node="archive"]');assert.equal(close.hidden,false);assert.equal(app.w.document.activeElement,close);
  close.click();assert.equal(close.hidden,true);assert.equal(app.w.document.activeElement.dataset.node,'archive');
});

test('projected labels fit the stage and avoid other node and condition text in all three cases',t=>{
  const app=boot(t,'design/rsi',false,false,true);
  for(const id of ['dgm','stop','aster']){
    app.click(`[data-case="${id}"]`);
    const boxes=[...app.widget.querySelectorAll('.hd-node,.hd-edge-label')].filter(n=>!n.hidden).map(n=>{
      const x=parseFloat(n.style.left),y=parseFloat(n.style.top);
      return {left:x-n.offsetWidth/2,right:x+n.offsetWidth/2,top:y-n.offsetHeight/2,bottom:y+n.offsetHeight/2,label:n.textContent};
    });
    for(const [i,a]of boxes.entries()){
      assert.ok(a.left>=0&&a.right<=620&&a.top>=0&&a.bottom<=650,`${id}: ${a.label} is in bounds`);
      for(const b of boxes.slice(i+1))assert.ok(a.right<=b.left||b.right<=a.left||a.bottom<=b.top||b.bottom<=a.top,`${id}: ${a.label} / ${b.label}`);
    }
  }
});

test('switching cases closes the lens and resets phases, nodes and source links together',t=>{
  const app=boot(t);app.click('[data-node="edit"]');app.click('[data-case="stop"]');
  assert.equal(app.widget.querySelector('.hd-lens').hidden,true);
  assert.equal(app.widget.querySelectorAll('[data-node]').length,9);
  assert.equal(app.widget.querySelector('[data-node="edit"]'),null);
  assert.equal(app.widget.querySelector('.hd-count').textContent,'1 / 5');
  app.click('[data-node="search"]');assert.match(app.widget.querySelector('.hd-detail-body').textContent,/束搜索/);
  app.click('[data-case="aster"]');app.click('[data-node="compose"]');
  assert.ok(app.widget.querySelector('.hd-detail-source').href.includes('/ba17a7ca69bb39bd6a98dce14377777951fa9119/'));
  assert.match(app.widget.querySelector('.hd-case-scope').textContent,/不是.*自主/);
  app.click('.hd-close');app.animations.at(-1).onfinish();app.click('[data-action="reset"]');app.click('[data-action="next"]');assert.equal(app.widget.querySelector('.hd-count').textContent,'2 / 5');
  for(let i=0;i<3;i++)app.click('[data-action="next"]');
  assert.equal(app.widget.querySelector('[data-action="next"]').disabled,true);
  app.click('[data-action="reset"]');assert.equal(app.widget.querySelector('.hd-count').textContent,'1 / 5');
  assert.equal(app.widget.querySelector('[data-action="previous"]').disabled,true);
});

test('every outer edge gets a moving trail and each internal connector gets a moving light point',t=>{
  const app=boot(t);
  assert.equal(app.frames.size,1);
  for(let i=0;i<24;i++)app.advance(1000+i*50);
  const data=JSON.parse(app.widget.dataset.example).cases[0];
  assert.equal(app.arcs().filter(a=>a[2]===1.9||a[2]===2.7).length,data.links.length,'one light head on every semantic edge');
  app.click('[data-node="edit"]');
  for(let i=0;i<24;i++)app.advance(2400+i*50);
  const dots=[...app.widget.querySelectorAll('[data-lens-kind="record"] .hd-particle')];
  assert.equal(dots.length,3);
  const before=dots.map(dot=>[dot.getAttribute('cx'),dot.getAttribute('cy')]);
  app.advance(3700);
  dots.forEach((dot,i)=>{assert.ok(Number(dot.style.opacity)>0);assert.notDeepEqual([dot.getAttribute('cx'),dot.getAttribute('cy')],before[i]);});
  app.click('[data-action="motion"]');assert.equal(app.frames.size,0);assert.equal(app.arcs().filter(a=>a[2]>=1.9).length,0);
  assert.ok(dots.every(dot=>dot.style.opacity==='0'));
});

test('node reading pauses narration independently; reduced motion keeps drill-down and safely finishes a closing lens',t=>{
  const app=boot(t);app.click('[data-action="play"]');assert.ok(app.widget.classList.contains('is-narrating'));assert.equal(app.frames.size,1);
  app.click('[data-node="edit"]');assert.equal(app.timers.size,0);assert.equal(app.frames.size,1);
  app.click('.hd-close');app.media.matches=true;app.changeMedia();
  assert.equal(app.widget.querySelector('.hd-lens').hidden,true);assert.equal(app.frames.size,0);
  const count=app.animations.length;app.click('[data-node="eval"]');assert.equal(app.animations.length,count);
  assert.match(app.widget.querySelector('.hd-detail-body').textContent,/编译.*编辑/);
  assert.equal(app.widget.querySelector('[data-action="motion"]').disabled,true);
  app.click('.hd-close');assert.equal(app.widget.querySelector('.hd-lens').hidden,true);
});

test('Core composition retains guard details and all animation work stops while the document is hidden',t=>{
  const app=boot(t,'meta-harness');app.click('[data-node="tools"]');
  assert.match(app.widget.querySelector('.hd-detail-body').textContent,/目录不是权限凭证/);
  assert.equal(app.widget.querySelectorAll('[data-lens-kind="record"]').length,3);
  app.click('[data-action="play"]');assert.equal(app.widget.querySelector('.hd-lens').hidden,false);assert.ok(app.widget.classList.contains('is-narrating'));
  Object.defineProperty(app.w.document,'hidden',{value:true,configurable:true});
  app.w.document.dispatchEvent(new app.w.Event('visibilitychange'));
  assert.equal(app.timers.size,0);assert.equal(app.frames.size,0);
});

test('leaving the viewport settles a lens transition and pauses drawing; returning resumes only visual flow',t=>{
  const app=boot(t);app.click('[data-action="play"]');app.click('[data-node="edit"]');app.click('.hd-close');
  app.intersect(false);
  assert.equal(app.widget.querySelector('.hd-lens').hidden,true);
  assert.equal(app.frames.size,0);assert.equal(app.timers.size,0);
  app.intersect(true);assert.equal(app.frames.size,1);assert.equal(app.timers.size,0);
  app.w.dispatchEvent(new app.w.Event('pagehide'));assert.equal(app.frames.size,0);
});

test('focus view keeps one graph in an accessible modal and restores it to the article on exit',t=>{
  const app=boot(t,'design/rsi',false,true),modal=app.w.document.querySelector('.hd-focus-view');
  assert.ok(modal.open);assert.equal(app.widget.parentElement,modal);
  assert.equal(modal.getAttribute('aria-label'),'RSI 演进图谱');
  const picker=app.widget.querySelector('.hd-case-picker');assert.equal(picker.open,false);
  app.click('.hd-case-picker summary');assert.equal(picker.open,true);
  app.click('[data-case="stop"]');assert.equal(picker.open,false);
  assert.match(app.widget.querySelector('.hd-case-name').textContent,/STOP/);
  app.click('[data-node="search"]');
  for(let i=0;i<24;i++)app.advance(1000+i*50);
  assert.equal(app.tetherArcs().length,2,'two flowing paths preserve the outer-to-inner connection');
  modal.dispatchEvent(new app.w.Event('cancel',{cancelable:true}));
  app.animations.at(-1).onfinish();assert.equal(app.widget.querySelector('.hd-lens').hidden,true);
  assert.equal(app.tetherArcs().length,0);
  modal.dispatchEvent(new app.w.Event('cancel',{cancelable:true}));
  assert.equal(app.w.document.querySelector('.hd-focus-view'),null);
  assert.ok(app.widget.closest('main'));
  assert.equal(app.w.document.activeElement,app.widget.querySelector('[data-action="focus"]'));
});

test('the current case has a connected, keyboard-navigable timeline that drives the graph and opens source-backed details',t=>{
  const app=boot(t,'design/rsi',true),history=app.widget.querySelector('.hd-transcript');
  history.open=true;history.dispatchEvent(new app.w.Event('toggle'));
  assert.ok(app.widget.classList.contains('is-history-open'));
  const record=history.querySelector('[data-record-case="dgm"]'),rows=[...record.querySelectorAll('[data-timeline-select]')];
  assert.equal(rows.length,6);
  assert.equal(history.querySelector('[data-record-case="stop"]').hidden,true);
  rows[2].click();
  assert.equal(rows[2].getAttribute('aria-current'),'step');
  assert.equal(rows[2].closest('.hd-event').open,true);
  assert.equal(rows[0].closest('.hd-event').open,false);
  assert.equal(app.widget.querySelector('[data-node="edit"]').classList.contains('is-focused'),true);
  assert.equal(history.querySelector('.hd-history-count').textContent,'3 / 6');
  rows[2].dispatchEvent(new app.w.KeyboardEvent('keydown',{key:'ArrowDown',bubbles:true}));
  assert.equal(app.w.document.activeElement,rows[3]);
  rows[2].closest('.hd-event').querySelector('[data-event-node]').click();
  assert.equal(history.open,false);assert.equal(app.widget.querySelector('.hd-lens').hidden,false);
  assert.match(app.widget.querySelector('.hd-detail-body').textContent,/字符串替换/);
  app.click('[data-case="aster"]');
  assert.equal(record.hidden,true);assert.equal(history.querySelector('[data-record-case="aster"]').hidden,false);
});

test('timeline expansion animates its height and settles immediately when reduced motion changes',t=>{
  const app=boot(t),history=app.widget.querySelector('.hd-transcript');history.open=true;history.dispatchEvent(new app.w.Event('toggle'));
  app.click('[data-record-case="dgm"] [data-timeline-select="3"]');
  const expansion=app.animations.find(a=>a.options.duration===400);
  assert.ok(expansion);assert.ok('height' in expansion.keyframes[0]);
  app.media.matches=true;app.changeMedia();
  assert.equal(expansion.cancelled,true);
  assert.equal(history.querySelector('[data-record-case="dgm"] [data-timeline-select="3"]').closest('.hd-event').open,true);
  assert.equal(app.frames.size,0);
});

test('the inner view keeps authored category branches separate from actual records',t=>{
  const app=boot(t);app.click('[data-node="edit"]');
  assert.deepEqual([...app.widget.querySelectorAll('.hd-branch-label')].map(n=>n.textContent),['工具变化','Aster 接入']);
  assert.equal(app.widget.querySelectorAll('[data-lens-kind="branch"]').length,2);
  assert.equal(app.widget.querySelectorAll('[data-lens-kind="record"]').length,3);
  assert.deepEqual([...app.widget.querySelectorAll('.hd-leaf-label')].map(n=>n.textContent),['改动前','改动后','Aster 接入映射']);
  assert.notEqual(app.widget.querySelector('[data-record="0"]').style.getPropertyValue('--leaf-angle'),'0deg');
  app.widget.querySelector('[data-record="0"]').dispatchEvent(new app.w.KeyboardEvent('keydown',{key:'End',bubbles:true}));
  assert.equal(app.w.document.activeElement,app.widget.querySelector('[data-record="2"]'));
});

test('replay presents one exact source record only when its path arrives, then emits one arrival ring',t=>{
  const app=boot(t),data=JSON.parse(app.widget.dataset.example).cases[0],seed=data.nodes.find(n=>n.id==='seed');
  app.click('[data-action="play"]');app.run(1500);
  assert.equal(app.widget.dataset.replayState,'travelling');assert.equal(app.widget.dataset.presentedRecord,undefined);
  assert.equal(app.widget.querySelector('.hd-detail-body').textContent,seed.detail);
  assert.ok(app.tetherArcs().some(a=>a[2]===3),'the active event has a travelling light head');
  app.run(1500);
  assert.equal(app.widget.dataset.replayState,'arrived');assert.equal(app.widget.dataset.presentedRecord,'0');
  assert.equal(app.widget.querySelector('.hd-detail-body').textContent,seed.children[0].detail);
  const rings=[...app.widget.querySelectorAll('.hd-arrival')];assert.equal(rings.filter(r=>Number(r.style.opacity)>0).length,1);
  app.run(2300);
  assert.equal(app.widget.dataset.record,'1');assert.equal(app.widget.dataset.presentedRecord,'0');
  app.run(2000);assert.equal(app.widget.dataset.presentedRecord,'1');assert.equal(app.widget.querySelector('.hd-detail-body').textContent,seed.children[1].detail);
});

test('pause freezes the active event and manual selection cancels automatic advancement',t=>{
  const app=boot(t);app.click('[data-action="play"]');app.run(1200);app.click('[data-action="play"]');
  const copy=app.widget.querySelector('.hd-detail-body').textContent;
  app.run(6000);assert.equal(app.widget.dataset.replayState,'paused');assert.equal(app.widget.querySelector('.hd-detail-body').textContent,copy);assert.equal(app.widget.dataset.presentedRecord,undefined);
  app.click('[data-action="play"]');app.run(2200);assert.equal(app.widget.dataset.presentedRecord,'0');
  app.click('[data-record="1"]');const selectedCopy=app.widget.querySelector('.hd-detail-body').textContent;
  assert.equal(app.widget.classList.contains('is-narrating'),false);assert.equal(app.widget.dataset.presentedRecord,'1');
  app.run(12000);assert.equal(app.widget.dataset.record,'1');assert.equal(app.widget.querySelector('.hd-detail-body').textContent,selectedCopy);
});

test('whole-case replay advances to the next phase and case switching cancels the old event',t=>{
  const app=boot(t);app.click('[data-action="play"]');
  for(let i=0;i<240&&app.widget.querySelector('.hd-count').textContent==='1 / 6';i++)app.run(50);
  assert.equal(app.widget.querySelector('.hd-count').textContent,'2 / 6');
  assert.equal(app.widget.querySelector('[data-node="parent"]').getAttribute('aria-expanded'),'true');
  assert.equal(app.widget.querySelector('[data-timeline-select="1"]').getAttribute('aria-current'),'step');
  app.click('[data-case="aster"]');app.run(12000);
  assert.equal(app.widget.querySelector('.hd-count').textContent,'1 / 5');assert.equal(app.widget.dataset.replayState,'idle');
  assert.equal(app.widget.querySelector('.hd-lens').hidden,true);assert.equal(app.widget.classList.contains('is-narrating'),false);
  assert.ok(app.widget.querySelector('.hd-detail-source').href.includes('ba17a7ca69bb39bd6a98dce14377777951fa9119'));
});

test('reduced motion advances a single requested record with no autoplay, light travel or ripple',t=>{
  const app=boot(t,'design/rsi',true);app.click('[data-action="play"]');
  assert.equal(app.widget.dataset.presentedRecord,'0');assert.equal(app.frames.size,0);assert.equal(app.timers.size,0);assert.equal(app.animations.length,0);
  assert.equal(app.widget.classList.contains('is-narrating'),false);
  assert.ok([...app.widget.querySelectorAll('.hd-arrival')].every(r=>Number(r.style.opacity)===0));
  app.click('[data-action="play"]');assert.equal(app.widget.dataset.presentedRecord,'1');
  app.click('[data-action="play"]');assert.equal(app.widget.querySelector('.hd-count').textContent,'2 / 6');assert.equal(app.widget.dataset.presentedRecord,'0');
});

test('manual node selection keeps the phase and history selection aligned without shrinking the label layer',t=>{
  const app=boot(t);app.click('[data-node="edit"]');
  assert.equal(app.widget.querySelector('.hd-count').textContent,'3 / 6');
  assert.equal(app.widget.querySelector('[data-timeline-select="2"]').getAttribute('aria-current'),'step');
  const labels=app.widget.querySelector('.hd-nodes');assert.equal(labels.parentElement,app.widget.querySelector('.hd-stage'));assert.equal(labels.inert,true);
  app.click('.hd-close');app.animations.at(-1).onfinish();assert.equal(labels.inert,false);
});

test('a finished node narration can restart from its first record',t=>{
  const app=boot(t);app.click('[data-node="edit"]');app.click('[data-action="play"]');app.run(16000);
  assert.equal(app.widget.dataset.replayState,'finished');assert.equal(app.widget.querySelector('[data-action="play"]').getAttribute('aria-label'),'重新讲解');
  app.click('[data-action="play"]');assert.equal(app.widget.dataset.record,'0');assert.equal(app.widget.dataset.replayState,'travelling');
});

test('authoring refuses missing or duplicate category membership before publishing',async()=>{
  const {attachGroups}=await import('../content/docs/code/9.1.6/_rsi-cases.mjs');
  const nodes=[{id:'item',children:[{label:'first'},{label:'second'}]}];
  assert.throws(()=>attachGroups(nodes,{}),/Missing relationship groups: item/);
  assert.throws(()=>attachGroups(nodes,{item:[{label:'group',kind:'evidence',records:[0,0]}]}),/cover each record once: item/);
  assert.throws(()=>attachGroups(nodes,{item:[{label:'group',kind:'evidence',records:[0,2]}]}),/cover each record once: item/);
});

test('each real case states its scenario, decisions, directed branches and outcomes explicitly',()=>{
  const dom=new JSDOM(fs.readFileSync(path.join(root,'apps/web/dist/docs/code/9.1.6/design/rsi/index.html'),'utf8'));
  const cases=JSON.parse(dom.window.document.querySelector('[data-harness-demo]').dataset.example).cases;
  for(const item of cases){
    for(const name of ['title','goal','baseline','decision','outcome'])assert.ok(item.scenario[name]);
    for(const role of ['scenario','decision','outcome'])assert.ok(item.nodes.some(n=>n.role===role));
    assert.equal(item.flow.length,item.links.length);
    for(const flow of item.flow){assert.ok(flow.label);assert.ok(item.nodes.some(n=>n.id===flow.from));assert.ok(item.nodes.some(n=>n.id===flow.to));}
    assert.ok(item.nodes.filter(n=>n.role==='decision').some(n=>item.flow.filter(f=>f.from===n.id).length>=2));
  }
  const dgm=cases.find(c=>c.id==='dgm');assert.ok(dgm.flow.some(f=>f.from==='eval'&&f.to==='archive'&&f.kind==='feedback'));
  assert.ok(dgm.flow.some(f=>f.from==='eval'&&f.to==='invalid'&&f.kind==='reject'));
  const stop=cases.find(c=>c.id==='stop');assert.deepEqual(stop.nodes.filter(n=>n.role==='branch').map(n=>n.id),['beam','genetic','anneal']);
  const aster=cases.find(c=>c.id==='aster');assert.deepEqual(aster.flow.filter(f=>f.from==='limit').map(f=>f.to),['unbounded','zero','finite']);
  dom.window.close();
});

test('a decision branch opens its sourced outcome and preserves the decision phase',t=>{
  const app=boot(t);const reject=app.widget.querySelector('.hd-edge-label[data-kind="reject"]');assert.ok(reject);assert.equal(reject.hidden,false);
  assert.match(app.widget.querySelector('.hd-scenario-title').textContent,/编码 Agent/);
  reject.click();assert.equal(app.widget.querySelector('[data-node="invalid"]').getAttribute('aria-expanded'),'true');
  assert.equal(app.widget.querySelector('.hd-count').textContent,'5 / 6');
  assert.match(app.widget.querySelector('.hd-detail-owner').textContent,/机制分支/);
  assert.match(app.widget.querySelector('.hd-detail-body').textContent,/编译失败或失去代码编辑能力/);
  assert.equal(new URL(app.widget.querySelector('.hd-detail-source').href).hostname,'arxiv.org');
  app.click('[data-case="aster"]');assert.match(app.widget.querySelector('.hd-scenario-title').textContent,/长任务/);
  const zero=[...app.widget.querySelectorAll('.hd-edge-label')].find(b=>b.textContent==='Some(0)');zero.click();
  assert.match(app.widget.querySelector('.hd-detail-body').textContent,/零工具额度.*阻止工具执行/);
  assert.equal(app.widget.querySelector('.hd-count').textContent,'3 / 5');
});

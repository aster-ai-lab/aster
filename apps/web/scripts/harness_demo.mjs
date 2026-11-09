// Version-owned evidence and narrated reading order; no harness runs in this view.
(() => {
  document.querySelectorAll('[data-harness-demo]').forEach(root => {
    let examples;
    try { examples=JSON.parse(root.dataset.example).cases; } catch { return; }
    const stage=root.querySelector('.hd-stage'),world=root.querySelector('.hd-world'),labelLayer=root.querySelector('.hd-nodes');
    const canvas=root.querySelector('canvas'),context=canvas.getContext('2d');
    const tether=root.querySelector('.hd-tether'),tetherContext=tether.getContext('2d');
    const lens=root.querySelector('.hd-lens'),circle=root.querySelector('.hd-lens-circle');
    const close=root.querySelector('.hd-close'),owner=root.querySelector('.hd-detail-owner');
    const title=root.querySelector('.hd-detail-title'),body=root.querySelector('.hd-detail-body'),source=root.querySelector('.hd-detail-source');
    const history=root.querySelector('.hd-transcript'),disclosures=new Map();
    const play=root.querySelector('[data-action="play"]'),motionButton=root.querySelector('[data-action="motion"]');
    const pill=root.querySelector('.hd-beat-pill'),reduced=window.matchMedia('(prefers-reduced-motion: reduce)');
    const immersive=root.dataset.mode==='rsi',center={x:200,y:200},positions=new Map();
    if(immersive){
      const toolbar=document.createElement('div'),picker=root.querySelector('.hd-case-picker');toolbar.className='hd-toolbar';picker.before(toolbar);
      toolbar.append(picker,close,root.querySelector('.hd-controls'));close.hidden=true;
    }
    const colors={component:'177,151,237',candidate:'173,221,93',decision:'173,221,93',session:'107,178,205',evidence:'226,179,101',failure:'239,126,143',guard:'186,207,166',policy:'185,196,203'};
    const field=Array.from({length:560},(_,i)=>{
      const y=1-(i+.5)/560*2,angle=i*2.3999632297,r=Math.sqrt(1-y*y);
      return [Math.cos(angle)*r,y,Math.sin(angle)*r];
    });
    const mesh=[];
    field.forEach((a,i)=>{for(let j=i+1;j<field.length;j++){const b=field[j];if((a[0]-b[0])**2+(a[1]-b[1])**2+(a[2]-b[2])**2<.045)mesh.push([i,j]);}});
    let example,nodes,links,steps,byId,anchors,edgeLabels=[],labelLeaders=[];
    let index=0,yaw=.18,pitch=-.16,playing=false,playScope=null,frame=0,selected=null,animation=null,closing=false,pointer=null;
    let branches=[],leaves=[],lensEdges=[],lensGeometry=null,activeRecord=null,beat=null,openedAt=0,cameraTarget=null,sceneClosingAt=0,sceneCloseFrom=0;
    let motion=true,visible=!window.IntersectionObserver,elapsed=0,lastTime=0,lastDraw=0,hovered=null,dialog=null,placeholder=null;
    const moving=()=>motion&&!reduced.matches&&visible&&!document.hidden;
    const animated=()=>motion&&!reduced.matches;

    function project([x,y,z],width,height) {
      const a=x*Math.cos(yaw)+z*Math.sin(yaw),b=z*Math.cos(yaw)-x*Math.sin(yaw);
      const c=y*Math.cos(pitch)-b*Math.sin(pitch),depth=y*Math.sin(pitch)+b*Math.cos(pitch);
      const radius=Math.min(width*.36,height*.40),perspective=3.6/(3.6-depth*.38);
      return {x:width*.5+a*radius*perspective,y:height*(immersive?.50:.42)-c*radius*perspective,depth};
    }
    function curve(a,b,t,bend) {
      const x=(a.x+b.x)/2-(b.y-a.y)*bend,y=(a.y+b.y)/2+(b.x-a.x)*bend;
      return {x:(1-t)**2*a.x+2*(1-t)*t*x+t*t*b.x,y:(1-t)**2*a.y+2*(1-t)*t*y+t*t*b.y};
    }
    function relationBend(index) {
      return example.flow?.[index]?.kind === 'feedback' ? .44 : index % 2 ? .12 : -.12;
    }
    function contextPoint(point,width,height) {
      const scale=immersive?.52:.58,dx=immersive?-.26:-.20,dy=immersive?.22:.18;
      return {x:width*(.5+dx)+(point.x-width*.5)*scale,y:height*(.5+dy)+(point.y-height*.5)*scale};
    }
    function sceneBlend() {
      if(!selected)return 0;
      if(!animated())return 1;
      const t=Math.min(1,Math.max(0,(elapsed-(closing?sceneClosingAt:openedAt))/(closing?520:880))),ease=1-(1-t)**3;
      return closing?sceneCloseFrom*(1-ease):ease;
    }
    function scenePoint(point,width,height) {
      const target=contextPoint(point,width,height),blend=sceneBlend();
      return {x:point.x+(target.x-point.x)*blend,y:point.y+(target.y-point.y)*blend,depth:point.depth};
    }
    function lensPoint(point) {
      return {x:lensGeometry.x+point.x/400*lensGeometry.size,y:lensGeometry.y+point.y/400*lensGeometry.size};
    }
    function measureLens() {
      if(!selected){lensGeometry=null;return;}
      if(immersive)stage.style.setProperty('--detail-space',`${root.querySelector('.hd-node-detail').offsetHeight+48}px`);
      const box=stage.getBoundingClientRect(),target=circle.getBoundingClientRect();
      lensGeometry={x:target.left-box.left,y:target.top-box.top,size:target.width};
      leaves.forEach(leaf=>{
        const width=Math.min(150,target.width*.34),x=leaf.point.x/400*target.width;
        leaf.button.style.maxWidth=`${width}px`;leaf.button.style.left=`${Math.max(width/2+10,Math.min(target.width-width/2-10,x))}px`;
      });
    }
    function drawLens() {
      const reveal=animated()?Math.min(1,(elapsed-openedAt)/750):1;
      for(const edge of lensEdges) {
        const hot=activeRecord!==null&&(edge.record===activeRecord||edge.record===null&&edge.group===leaves[activeRecord].group);
        edge.element.classList.toggle('is-hot',hot);
        const length=Math.hypot(edge.to.x-edge.from.x,edge.to.y-edge.from.y)*1.08;
        edge.path.style.strokeDasharray=String(length);edge.path.style.strokeDashoffset=String(length*(1-reveal));
        const t=(elapsed/(hot?3000:5300)+edge.group*.19+(edge.record??0)*.13)%1,p=curve(edge.from,edge.to,t,edge.bend);
        edge.dot.style.opacity=animated()&&reveal===1&&!(hot&&beat&&!beat.arrived)?hot?'.8':'.28':'0';
        edge.dot.setAttribute('cx',p.x);edge.dot.setAttribute('cy',p.y);
      }
      leaves.forEach((leaf,i)=>{
        const hot=i===activeRecord;leaf.mark.classList.toggle('is-hot',hot);leaf.button.classList.toggle('is-hot',hot);
        const age=beat&&hot&&beat.arrived?beat.age-beat.travel:Infinity,t=age/700;
        leaf.ring.setAttribute('r',String(5+Math.min(1,t)*17));
        leaf.ring.style.opacity=animated()&&t>=0&&t<1?String((1-t)*.8):'0';
      });
    }
    function placeLabels(items,width,height) {
      const occupied=[],offsets=[[0,0]];labelLeaders=[];
      for(const distance of [28,52,82,118,160])offsets.push([0,-distance],[0,distance],[-distance,0],[distance,0],[-distance,-distance],[distance,-distance],[-distance,distance],[distance,distance]);
      const measured=items.map(item=>({...item,width:item.element.offsetWidth,height:item.element.offsetHeight})).sort((a,b)=>b.priority-a.priority);
      for(const item of measured){
        const halfW=item.width/2,halfH=item.height/2;
        let best={x:item.x,y:item.y},score=Infinity;
        for(const [dx,dy]of offsets){
          const x=Math.max(halfW+10,Math.min(width-halfW-10,item.x+dx)),y=Math.max(halfH+14,Math.min(height-halfH-14,item.y+dy));
          const overlap=occupied.reduce((sum,box)=>sum+Math.max(0,Math.min(x+halfW+5,box.right)-Math.max(x-halfW-5,box.left))*Math.max(0,Math.min(y+halfH+5,box.bottom)-Math.max(y-halfH-5,box.top)),0);
          const cost=overlap*10000+(x-item.x)**2+(y-item.y)**2;
          if(cost<score){score=cost;best={x,y};}if(cost===0)break;
        }
        item.element.style.left=`${best.x}px`;item.element.style.top=`${best.y}px`;
        occupied.push({left:best.x-halfW,right:best.x+halfW,top:best.y-halfH,bottom:best.y+halfH});
        if(Math.hypot(best.x-item.x,best.y-item.y)>8)labelLeaders.push({from:item,to:best});
      }
    }
    function draw() {
      const width=stage.clientWidth||560,height=stage.clientHeight||550,active=new Set(steps[index].active),labels=[];
      for(const node of nodes) {
        const p=project(node.point,width,height),anchor=anchors.get(node.id);positions.set(node.id,p);
        const labelPoint=scenePoint(p,width,height);anchor.style.left=`${labelPoint.x}px`;anchor.style.top=`${labelPoint.y}px`;anchor.style.zIndex=String(Math.round((p.depth+1)*10));
        anchor.classList.toggle('is-active',active.has(node.id));anchor.classList.toggle('is-focused',(selected||steps[index].focus)===node.id);
        if(immersive)labels.push({element:anchor,...labelPoint,priority:(selected||steps[index].focus)===node.id?3:active.has(node.id)?2:1});
      }
      edgeLabels.forEach(({element,info,i})=>{
        const a=scenePoint(positions.get(info.from),width,height),b=scenePoint(positions.get(info.to),width,height),p=curve(a,b,.43,relationBend(i));
        element.style.left=`${p.x}px`;element.style.top=`${p.y-9}px`;
        element.hidden=selected!==null||(!info.visible&&!(hovered===info.from||hovered===info.to));
        if(!element.hidden)labels.push({element,x:p.x,y:p.y-9,priority:0});
      });
      if(immersive)placeLabels(labels,width,height);
      drawLens();drawTether(width,height);
      if(!context)return;
      const ratio=window.devicePixelRatio||1;
      if(canvas.width!==Math.round(width*ratio)||canvas.height!==Math.round(height*ratio)){canvas.width=Math.round(width*ratio);canvas.height=Math.round(height*ratio);}
      context.setTransform(ratio,0,0,ratio,0,0);context.clearRect(0,0,width,height);
      const projected=field.map(p=>scenePoint(project(p,width,height),width,height));
      mesh.forEach(([from,to],i)=>{
        const a=projected[from],b=projected[to],depth=(a.depth+b.depth+2)/4;
        context.beginPath();context.moveTo(a.x,a.y);context.lineTo(b.x,b.y);context.strokeStyle=`rgba(145,173,181,${.006+depth*.026})`;context.lineWidth=.45;context.stroke();
        if(animated()&&i%3===0){const t=(elapsed/(7800+i%7*900)+i*.271)%1;context.beginPath();context.arc(a.x+(b.x-a.x)*t,a.y+(b.y-a.y)*t,.55,0,Math.PI*2);context.fillStyle=`rgba(166,197,196,${.08+depth*.2})`;context.fill();}
      });
      projected.forEach((p,i)=>{
        const depth=(p.depth+1)/2;context.beginPath();context.arc(p.x,p.y,.45+depth*.8,0,Math.PI*2);
        context.fillStyle=`rgba(${i%17===0?'176,205,138':i%11===0?'167,147,191':'160,188,197'},${.09+depth*.25})`;context.fill();
      });
      for(const {from,to}of labelLeaders){
        context.beginPath();context.moveTo(from.x,from.y);context.lineTo(to.x,to.y);context.strokeStyle='rgba(179,199,214,.25)';context.lineWidth=.65;context.setLineDash([2,3]);context.stroke();context.setLineDash([]);
      }
      links.forEach(([from,to],i)=>{
        const a=scenePoint(positions.get(from),width,height),b=scenePoint(positions.get(to),width,height),lit=hovered?from===hovered||to===hovered:active.has(from)&&active.has(to),bend=relationBend(i),info=example.flow?.[i],color=info?.kind==='reject'?colors.failure:info?.kind==='feedback'?colors.candidate:colors[byId.get(to).kind]||colors.policy;
        context.beginPath();context.moveTo(a.x,a.y);
        for(let n=1;n<=24;n++){const p=curve(a,b,n/24,bend);context.lineTo(p.x,p.y);}
        context.setLineDash(info?.kind==='reject'?[5,4]:info?.kind==='evidence'?[2,4]:[]);
        context.strokeStyle=`rgba(${color},${lit?.8:.32})`;context.lineWidth=lit?1.25:.8;context.stroke();context.setLineDash([]);
        if(info){
          const tip=curve(a,b,.61,bend),tail=curve(a,b,.58,bend),angle=Math.atan2(tip.y-tail.y,tip.x-tail.x),size=4.2;
          context.beginPath();context.moveTo(tip.x+Math.cos(angle)*size,tip.y+Math.sin(angle)*size);context.lineTo(tip.x-Math.cos(angle-.6)*size,tip.y-Math.sin(angle-.6)*size);context.lineTo(tip.x-Math.cos(angle+.6)*size,tip.y-Math.sin(angle+.6)*size);context.fillStyle=`rgba(${color},.9)`;context.fill();
        }
        if(animated()){
          const t=(elapsed/(3100+i%3*650)+i*.137)%1;
          for(let trail=4;trail>=0;trail--){const p=curve(a,b,(t-trail*.014+1)%1,bend);context.beginPath();context.arc(p.x,p.y,trail?.8:lit?2.7:1.9,0,Math.PI*2);context.fillStyle=`rgba(${color},${trail?lit?.18:.08:lit?1:.7})`;context.shadowColor=`rgb(${color})`;context.shadowBlur=trail?0:12;context.fill();context.shadowBlur=0;}
        }
      });
    }
    function drawTether(width,height) {
      if(!tetherContext)return;
      const ctx=tetherContext,ratio=window.devicePixelRatio||1;
      if(tether.width!==Math.round(width*ratio)||tether.height!==Math.round(height*ratio)){tether.width=Math.round(width*ratio);tether.height=Math.round(height*ratio);}
      ctx.setTransform(ratio,0,0,ratio,0,0);ctx.clearRect(0,0,width,height);
      if(!selected||!lensGeometry)return;
      const origin=scenePoint(positions.get(selected),width,height);
      branches.slice(0,2).forEach(branch=>{
        const to=lensPoint(leaves[branch.records[0]].point),hot=activeRecord!==null&&leaves[activeRecord].group===branch.index;
        ctx.beginPath();ctx.moveTo(origin.x,origin.y);
        for(let n=1;n<=50;n++){const p=curve(origin,to,n/50,-.16);ctx.lineTo(p.x,p.y);}
        ctx.strokeStyle=`rgba(${branch.color},${hot?.48:.16})`;ctx.lineWidth=.8;ctx.stroke();
        if(animated()&&!beat){const p=curve(origin,to,(elapsed/5200+branch.index*.3)%1,-.16);ctx.beginPath();ctx.arc(p.x,p.y,1.6,0,Math.PI*2);ctx.fillStyle=`rgba(${branch.color},.6)`;ctx.fill();}
      });
      if(!beat||beat.age<0)return;
      const leaf=leaves[beat.record],branch=branches[leaf.group],color=branch.color,route=playScope==='case'&&beat.record===0?(steps[index].route||[selected]):[selected];
      const path=[];
      for(let i=1;i<route.length;i++)path.push({a:scenePoint(positions.get(route[i-1]),width,height),b:scenePoint(positions.get(route[i]),width,height),bend:relationBend(links.findIndex(([from,to])=>from===route[i-1]&&to===route[i]))});
      path.push({a:origin,b:lensPoint(center),bend:-.13},{a:lensPoint(center),b:lensPoint(branch.point),bend:0},{a:lensPoint(branch.point),b:lensPoint(leaf.point),bend:.13});
      const total=path.reduce((sum,edge)=>sum+Math.hypot(edge.b.x-edge.a.x,edge.b.y-edge.a.y),0)||1;
      let travelled=Math.min(1,beat.age/beat.travel)*total,head=path[0].a;
      for(const edge of path){
        const length=Math.hypot(edge.b.x-edge.a.x,edge.b.y-edge.a.y),fraction=length?Math.min(1,Math.max(0,travelled/length)):1;
        ctx.beginPath();ctx.moveTo(edge.a.x,edge.a.y);
        for(let n=1;n<=32;n++){const p=curve(edge.a,edge.b,n/32*fraction,edge.bend);ctx.lineTo(p.x,p.y);}
        ctx.strokeStyle=`rgba(${color},${beat.arrived?.36:.85})`;ctx.lineWidth=1.25;ctx.stroke();
        if(travelled>=0)head=curve(edge.a,edge.b,fraction,edge.bend);travelled-=length;
      }
      if(animated()&&!beat.arrived){ctx.beginPath();ctx.arc(head.x,head.y,3,0,Math.PI*2);ctx.fillStyle=`rgb(${color})`;ctx.shadowColor=`rgb(${color})`;ctx.shadowBlur=16;ctx.fill();ctx.shadowBlur=0;ctx.beginPath();ctx.arc(head.x,head.y,1.15,0,Math.PI*2);ctx.fillStyle='#f4f8ee';ctx.fill();}
    }
    function makeSvg(tag,attrs,parent) {
      const element=document.createElementNS('http://www.w3.org/2000/svg',tag);
      for(const [name,value]of Object.entries(attrs))element.setAttribute(name,String(value));
      parent.append(element);return element;
    }
    function buildLens(node) {
      const edgeHolder=circle.querySelector('.hd-lens-edges'),markHolder=circle.querySelector('.hd-lens-marks'),labels=root.querySelector('.hd-lens-nodes');
      edgeHolder.replaceChildren();markHolder.replaceChildren();labels.replaceChildren();branches=[];leaves=[];lensEdges=[];
      makeSvg('circle',{cx:200,cy:200,r:4.5,class:'hd-root-mark'},markHolder);makeSvg('circle',{cx:200,cy:200,r:7.5,class:'hd-root-ring'},markHolder);
      const angles=node.groups.length===1?[-.55]:node.groups.length===2?[-2.05,.82]:[-2.5,-.35,1.5];
      const addEdge=(from,to,bend,group,record)=>{
        const color=branches[group].color,element=makeSvg('g',{'data-lens-edge':lensEdges.length,'data-lens-kind':record===null?'branch':'record','data-group':group},edgeHolder);
        element.style.setProperty('--edge-color',`rgb(${color})`);
        const control={x:(from.x+to.x)/2-(to.y-from.y)*bend,y:(from.y+to.y)/2+(to.x-from.x)*bend};
        const path=makeSvg('path',{d:`M${from.x} ${from.y} Q${control.x} ${control.y} ${to.x} ${to.y}`},element);
        const dot=makeSvg('circle',{class:'hd-particle',cx:from.x,cy:from.y,r:1.7},element);
        lensEdges.push({element,path,dot,from,to,bend,group,record});
      };
      node.groups.forEach((group,i)=>{
        const angle=angles[i],radius=65+(i%2)*8,point={x:200+Math.cos(angle)*radius,y:200+Math.sin(angle)*radius},color=colors[group.kind]||colors.policy;
        const branch={...group,index:i,point,color};branches.push(branch);addEdge(center,point,0,i,null);
        makeSvg('circle',{cx:point.x,cy:point.y,r:2.3,class:'hd-branch-mark'},markHolder);
        const label=document.createElement('span');label.className='hd-branch-label';label.id=`${root.id}-branch-${i}`;label.textContent=group.label;label.style.left=`${point.x/4}%`;label.style.top=`${point.y/4}%`;label.style.setProperty('--branch-color',`rgb(${color})`);labels.append(label);
        group.records.forEach((record,j)=>{
          const spread=group.records.length===1?0:node.groups.length===1?1.45:.65,a=angle+(j-(group.records.length-1)/2)*spread;
          const radius=135+(record%3)*9,point={x:200+Math.cos(a)*radius,y:200+Math.sin(a)*radius};addEdge(branch.point,point,.13,i,record);
          const mark=makeSvg('circle',{cx:point.x,cy:point.y,r:3.1,class:'hd-record-mark','data-record-mark':record},markHolder);mark.style.setProperty('--record-color',`rgb(${color})`);
          const ring=makeSvg('circle',{cx:point.x,cy:point.y,r:5,class:'hd-arrival'},markHolder);ring.style.setProperty('--record-color',`rgb(${color})`);
          const button=document.createElement('button');button.type='button';button.className='hd-leaf-label';button.textContent=node.children[record].label;button.dataset.record=record;button.setAttribute('aria-pressed','false');button.setAttribute('aria-describedby',label.id);
          button.style.left=`${point.x/4}%`;button.style.top=`${point.y/4}%`;
          button.style.setProperty('--record-color',`rgb(${color})`);
          button.addEventListener('click',()=>{stopNarration();playScope=null;beginRecord(record,true);});
          button.addEventListener('pointerenter',()=>{if(playing)stopNarration();});button.addEventListener('focus',()=>{if(playing)stopNarration();});
          button.addEventListener('keydown',event=>{
            if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Home','End'].includes(event.key))return;event.preventDefault();
            const next=event.key==='Home'?0:event.key==='End'?node.children.length-1:Math.max(0,Math.min(node.children.length-1,record+(['ArrowLeft','ArrowUp'].includes(event.key)?-1:1)));
            leaves[next].button.focus();
          });
          labels.append(button);leaves[record]={point,group:i,mark,ring,button};
        });
      });
    }
    function controls() {
      const recordMode=selected!==null,total=recordMode?byId.get(selected).children.length:steps.length,current=recordMode?(activeRecord??0):index;
      const previous=root.querySelector('[data-action="previous"]'),next=root.querySelector('[data-action="next"]');
      previous.disabled=current===0;next.disabled=recordMode&&activeRecord===null?false:current===total-1;
      previous.setAttribute('aria-label',recordMode?'上一条记录':'上一个阶段');previous.title=previous.getAttribute('aria-label');
      next.setAttribute('aria-label',recordMode?'下一条记录':'下一个阶段');next.title=next.getAttribute('aria-label');
      const atEnd=recordMode&&activeRecord===total-1&&(playScope!=='case'||index===steps.length-1);
      const finished=beat&&beat.arrived&&activeRecord===total-1&&beat.age>=beat.travel+2400;
      play.textContent=reduced.matches?(atEnd?'重新讲解':'下一条讲解'):playing?'暂停讲解':finished?'重新讲解':beat?'继续讲解':'开始讲解';play.setAttribute('aria-label',play.textContent);play.title=play.textContent;
      root.classList.toggle('is-narrating',playing);
    }
    function presentRecord(arrived=true) {
      const node=byId.get(selected),child=node.children[beat.record];describe(node,child);measureLens();if(arrived)beat.arrived=true;
      root.dataset.presentedRecord=String(beat.record);root.dataset.replayState=beat.arrived?'arrived':'travelling';
      pill.textContent=`讲解记录 ${beat.record+1} / ${node.children.length} · ${child.label}`;
    }
    function beginRecord(record,manual=false) {
      activeRecord=record;
      const intro=animation&&animated()?Math.max(0,880-(elapsed-openedAt)):0;
      beat={record,age:-intro,travel:1900,arrived:false,manual};
      root.dataset.record=String(record);root.dataset.replayState='travelling';pill.hidden=false;
      pill.textContent=`沿关系定位 · ${byId.get(selected).children[record].label}`;
      leaves.forEach((leaf,i)=>leaf.button.setAttribute('aria-pressed',String(i===record)));
      if(manual)presentRecord(false);if(!animated()){beat.age=beat.travel+700;presentRecord();}
      controls();draw();
    }
    function advanceReplay() {
      const count=byId.get(selected).children.length;
      if(activeRecord!==null&&activeRecord<count-1){beginRecord(activeRecord+1);return;}
      if(playScope==='case'&&index<steps.length-1){showStep(index+1);openNode(steps[index].focus,true);beginRecord(0);return;}
      stopNarration();root.dataset.replayState='finished';pill.textContent=`讲解已结束 · ${byId.get(selected).children[activeRecord].label}`;
    }
    function updateReplay(delta) {
      if(!beat||(!playing&&!beat.manual))return;
      beat.age+=delta;
      if(!beat.arrived&&beat.age>=beat.travel)presentRecord();
      if(beat.manual&&beat.age>=beat.travel+700)beat.manual=false;
      if(playing&&beat.age>=beat.travel+2400)advanceReplay();
    }
    function startReplay() {
      if(playing){stopNarration();return;}
      if(beat&&beat.arrived&&activeRecord===byId.get(selected).children.length-1&&beat.age>=beat.travel+2400){if(playScope==='case'){showStep(0);openNode(steps[0].focus,true);}beginRecord(0);}
      if(!selected){playScope='case';openNode(steps[index].focus,true);}
      else if(!playScope)playScope='node';
      if(reduced.matches){
        if(!beat)beginRecord(0,true);
        else if(activeRecord<byId.get(selected).children.length-1)beginRecord(activeRecord+1,true);
        else if(playScope==='case'&&index<steps.length-1){showStep(index+1);openNode(steps[index].focus,true);beginRecord(0,true);}
        else{if(playScope==='case'){showStep(0);openNode(steps[0].focus,true);}beginRecord(0,true);}
        controls();return;
      }
      if(!motion){motion=true;syncMotion();}
      if(!beat)beginRecord(0);else if(beat.arrived)beat.age=beat.travel;
      playing=true;beat.manual=false;root.dataset.replayState=beat.arrived?'arrived':'travelling';controls();
    }
    function tick(time) {
      frame=0;if(!moving())return;
      if(lastTime){
        const delta=Math.min(50,time-lastTime);elapsed+=delta;
        if(cameraTarget&&!pointer&&!hovered){const settle=1-Math.exp(-delta*.006);yaw+=(cameraTarget.yaw-yaw)*settle;pitch+=(cameraTarget.pitch-pitch)*settle;if(Math.abs(cameraTarget.yaw-yaw)+Math.abs(cameraTarget.pitch-pitch)<.001)cameraTarget=null;}
        updateReplay(delta);
      }
      lastTime=time;if(time-lastDraw>=32){lastDraw=time;draw();}frame=requestAnimationFrame(tick);
    }
    function syncMotion() {
      cancelAnimationFrame(frame);frame=0;lastTime=0;
      root.classList.toggle('is-motion-paused',!animated());motionButton.disabled=reduced.matches;motionButton.setAttribute('aria-pressed',String(animated()));
      motionButton.textContent=reduced.matches?'已减少动效':motion?'暂停动效':'开启动效';motionButton.setAttribute('aria-label',motionButton.textContent);motionButton.title=motionButton.textContent;
      if(moving())frame=requestAnimationFrame(tick);else settleDisclosures();
      controls();draw();
    }
    function stopNarration() {
      playing=false;if(beat)beat.manual=false;root.dataset.replayState=beat?'paused':'idle';if(steps)controls();
    }
    function describe(node,child) {
      owner.textContent=node.owner;title.textContent=child?`${node.label} / ${child.label}`:node.label;body.textContent=child?child.detail:node.detail;source.href=node.source;
    }
    function clearLens(returnFocus=false) {
      if(animation){animation.cancel();animation=null;}
      lens.hidden=true;world.inert=false;labelLayer.inert=false;stage.classList.remove('is-peeking');closing=false;
      if(immersive)close.hidden=true;
      if(selected){const anchor=anchors.get(selected);anchor.setAttribute('aria-expanded','false');if(returnFocus)anchor.focus();}
      selected=null;lensGeometry=null;activeRecord=null;beat=null;lensEdges=[];branches=[];leaves=[];pill.hidden=true;
      delete root.dataset.record;delete root.dataset.presentedRecord;root.dataset.replayState='idle';root.classList.remove('has-detail');
      if(tetherContext)tetherContext.clearRect(0,0,tether.width,tether.height);
    }
    function settleTransition() {
      if(closing){clearLens(true);controls();}
      else if(animation){animation.cancel();animation=null;}
    }
    function fly(opening,finish) {
      const node=positions.get(selected),box=stage.getBoundingClientRect(),target=circle.getBoundingClientRect();
      const dx=box.left+node.x-target.left-target.width/2,dy=box.top+node.y-target.top-target.height/2,origin=`translate(${dx}px,${dy}px) scale(.035)`;
      const current=getComputedStyle(circle),from=opening?origin:current.transform||'none',opacity=opening?.12:current.opacity||1;
      if(animation)animation.cancel();
      if(!animated()||!circle.animate){finish();return;}
      animation=circle.animate([{transform:from,opacity},{transform:opening?'none':origin,opacity:opening?1:0}],{duration:opening?880:520,easing:'cubic-bezier(.16,1,.3,1)'});
      animation.onfinish=()=>{animation=null;finish();};
    }
    function openNode(id,fromReplay=false) {
      if(!fromReplay){stopNarration();playScope=null;const phase=steps.findIndex(step=>step.focus===id);if(phase>=0&&phase!==index)showStep(phase);cameraTarget=null;}
      clearLens();if(immersive){history.open=false;root.classList.remove('is-history-open');}selected=id;
      const node=byId.get(id);describe(node);openedAt=elapsed;
      root.classList.add('has-detail');root.querySelector('.hd-lens-title').textContent=`内部 · ${node.label}`;buildLens(node);
      root.style.setProperty('--lens-rgb',colors[node.kind]||colors.policy);if(immersive)close.hidden=false;
      lens.hidden=false;anchors.get(id).setAttribute('aria-expanded','true');world.inert=true;labelLayer.inert=true;measureLens();draw();
      fly(true,()=>{measureLens();draw();});stage.classList.add('is-peeking');if(!fromReplay)close.focus();controls();
    }
    function closeNode() {
      if(!selected||closing)return;stopNarration();playScope=null;sceneCloseFrom=sceneBlend();sceneClosingAt=elapsed;closing=true;stage.classList.remove('is-peeking');
      fly(false,()=>{clearLens(true);controls();draw();});
    }
    function settleDisclosures() {
      disclosures.forEach(({animation,open},detail)=>{animation.cancel();detail.open=open;detail.style.overflow='';});disclosures.clear();
    }
    function setEventOpen(detail,open,animate=true) {
      const previous=disclosures.get(detail),from=detail.getBoundingClientRect().height;
      if(previous)previous.animation.cancel();disclosures.delete(detail);
      if(!animate||!motion||reduced.matches||!detail.animate){detail.open=open;detail.style.overflow='';return;}
      if(detail.open===open&&!previous)return;
      detail.open=true;
      const to=open?detail.scrollHeight:detail.querySelector('summary').getBoundingClientRect().height;
      detail.style.overflow='hidden';
      const animation=detail.animate([{height:`${from}px`},{height:`${to}px`}],{duration:400,easing:'cubic-bezier(.16,1,.3,1)'});
      disclosures.set(detail,{animation,open});animation.onfinish=()=>{detail.open=open;detail.style.overflow='';disclosures.delete(detail);};
    }
    function syncHistory(expand=true) {
      history.querySelectorAll('[data-record-case]').forEach(record=>{
        const current=record.dataset.recordCase===example.id;record.hidden=!current;
        if(!current)return;
        record.querySelectorAll('.hd-timeline-step').forEach((step,i)=>{
          step.classList.toggle('is-before',i<index);
          const summary=step.querySelector('[data-timeline-select]');
          if(i===index)summary.setAttribute('aria-current','step');else summary.removeAttribute('aria-current');
          if(expand)setEventOpen(step.querySelector('.hd-event'),i===index,history.open);
        });
      });
      history.querySelector('.hd-history-count').textContent=`${index+1} / ${steps.length}`;
    }
    function showStep(next,expandHistory=true) {
      clearLens();index=next;
      root.querySelectorAll('[data-step]').forEach((item,i)=>{if(i===index)item.setAttribute('aria-current','step');else item.removeAttribute('aria-current');});
      root.querySelector('.hd-count').textContent=`${index+1} / ${steps.length}`;root.querySelector('.hd-result').textContent=steps[index].result;
      owner.textContent=example.type;title.textContent=steps[index].title;body.textContent=steps[index].body;source.href=byId.get(steps[index].focus).source;
      const [x,y,z]=byId.get(steps[index].focus).point;
      let difference=Math.atan2(-x,z)-yaw;difference=Math.atan2(Math.sin(difference),Math.cos(difference));
      cameraTarget={yaw:yaw+difference,pitch:Math.atan2(y,Math.hypot(x,z))*.7};
      if(!animated()){yaw=cameraTarget.yaw;pitch=cameraTarget.pitch;cameraTarget=null;}
      syncHistory(expandHistory);controls();draw();
    }
    function chooseCase(item) {
      stopNarration();playScope=null;settleDisclosures();clearLens();example=item;({nodes,links,steps}=item);byId=new Map(nodes.map(n=>[n.id,n]));anchors=new Map();positions.clear();yaw=.18;pitch=-.16;
      const name=root.querySelector('.hd-case-name');if(name)name.textContent=item.title;
      const picker=root.querySelector('.hd-case-picker');if(picker)picker.open=false;
      if(item.scenario){root.querySelector('.hd-scenario-title').textContent=item.scenario.title;root.querySelector('.hd-scenario-goal').textContent=`目标：${item.scenario.goal}`;}
      root.querySelector('.hd-case-type').textContent=item.type;root.querySelector('.hd-case-scope').textContent=item.scope;
      root.querySelectorAll('[data-case]').forEach(a=>{if(a.dataset.case===item.id)a.setAttribute('aria-current','true');else a.removeAttribute('aria-current');});
      const menu=root.querySelector('.hd-stages'),holder=root.querySelector('.hd-nodes');menu.replaceChildren();holder.replaceChildren();
      steps.forEach((step,i)=>{
        const a=document.createElement('a'),num=document.createElement('span');a.href=`#${root.id}-${item.id}-step-${i}`;a.dataset.step=i;num.textContent=String(i+1).padStart(2,'0');a.append(num,step.title);
        a.addEventListener('click',event=>{event.preventDefault();stopNarration();playScope=null;showStep(i);});menu.append(a);
      });
      nodes.forEach(node=>{
        const a=document.createElement('a'),dot=document.createElement('i'),label=document.createElement('span');
        a.href=`#${root.id}-${item.id}-node-${node.id}`;a.className='hd-node';a.dataset.node=node.id;a.setAttribute('aria-label',node.label);a.dataset.kind=node.kind;if(node.role)a.dataset.role=node.role;a.setAttribute('aria-controls',lens.id);a.setAttribute('aria-expanded','false');
        dot.setAttribute('aria-hidden','true');label.textContent=node.label;a.append(dot);if(node.roleLabel){const role=document.createElement('span');role.className='hd-node-role';role.textContent=node.roleLabel;a.append(role);}a.append(label);a.addEventListener('click',event=>{event.preventDefault();openNode(node.id);});
        a.addEventListener('pointerenter',()=>{hovered=node.id;draw();});a.addEventListener('pointerleave',()=>{hovered=null;draw();});
        a.addEventListener('focus',()=>{hovered=node.id;draw();});a.addEventListener('blur',()=>{hovered=null;draw();});holder.append(a);anchors.set(node.id,a);
      });
      edgeLabels=(item.flow||[]).map((info,i)=>{
        const element=document.createElement('button');element.type='button';element.className='hd-edge-label';element.dataset.kind=info.kind;element.dataset.edge=i;element.textContent=info.label;
        element.setAttribute('aria-label',`${byId.get(info.from).label}，${info.label}，查看${byId.get(info.to).label}`);element.title=`查看关系：${info.label}`;
        element.addEventListener('click',()=>{const phase=steps.findIndex(step=>step.focus===info.from);if(phase>=0){stopNarration();showStep(phase);}openNode(info.to);});holder.append(element);return {element,info,i};
      });
      hovered=null;showStep(0);cameraTarget=null;
    }
    function toggleFocus() {
      stopNarration();settleTransition();
      const button=root.querySelector('[data-action="focus"]');
      if(dialog){
        const old=dialog;dialog=null;placeholder.replaceWith(root);placeholder=null;old.close();old.remove();button.setAttribute('aria-label','专注查看图谱');button.title='专注查看图谱';button.focus();
      }else{
        placeholder=document.createComment('harness graph');root.before(placeholder);
        dialog=document.createElement('dialog');dialog.className='hd-focus-view prose';dialog.setAttribute('aria-label','RSI 演进图谱');document.body.append(dialog);dialog.append(root);
        dialog.addEventListener('cancel',event=>{event.preventDefault();if(selected)closeNode();else if(history.open){history.open=false;history.querySelector('summary').focus();}else toggleFocus();});
        dialog.showModal();button.setAttribute('aria-label','返回文档');button.title='返回文档';button.focus();
      }
      measureLens();draw();
    }
    history.addEventListener('toggle',()=>{root.classList.toggle('is-history-open',history.open);if(history.open){stopNarration();playScope=null;clearLens();syncHistory();controls();draw();}});
    history.querySelectorAll('[data-timeline-select]').forEach(summary=>{
      summary.addEventListener('click',event=>{
        event.preventDefault();stopNarration();playScope=null;
        const detail=summary.closest('.hd-event'),wasOpen=detail.open,next=Number(summary.dataset.timelineSelect);showStep(next,false);
        summary.closest('[data-record-case]').querySelectorAll('.hd-event').forEach(item=>setEventOpen(item,item===detail?!wasOpen:false));
      });
      summary.addEventListener('keydown',event=>{
        if(!['ArrowDown','ArrowUp','Home','End'].includes(event.key))return;event.preventDefault();
        const rows=[...summary.closest('[data-record-case]').querySelectorAll('[data-timeline-select]')],position=rows.indexOf(summary);
        rows[event.key==='Home'?0:event.key==='End'?rows.length-1:Math.max(0,Math.min(rows.length-1,position+(event.key==='ArrowDown'?1:-1)))].focus();
      });
    });
    history.querySelectorAll('[data-event-node]').forEach(anchor=>anchor.addEventListener('click',event=>{event.preventDefault();openNode(anchor.dataset.eventNode);}));
    root.querySelectorAll('[data-case]').forEach(a=>a.addEventListener('click',event=>{event.preventDefault();chooseCase(examples.find(item=>item.id===a.dataset.case));}));
    close.addEventListener('click',closeNode);
    root.addEventListener('keydown',event=>{if(event.key!=='Escape')return;if(selected){event.preventDefault();event.stopPropagation();closeNode();}else if(history.open){event.preventDefault();event.stopPropagation();history.open=false;history.querySelector('summary').focus();}});
    root.querySelector('.hd-controls').hidden=false;
    root.querySelectorAll('[data-action]').forEach(button=>{
      button.setAttribute('aria-label',button.getAttribute('aria-label')||button.textContent);button.title=button.getAttribute('aria-label');
      button.addEventListener('click',()=>{
        if(button.dataset.action==='focus'){toggleFocus();return;}
        if(button.dataset.action==='motion'){
          motion=!motion;stopNarration();settleTransition();if(!motion&&beat&&!beat.arrived){beat.age=beat.travel+700;presentRecord();}syncMotion();return;
        }
        if(button.dataset.action==='play'){startReplay();return;}
        stopNarration();playScope=null;
        if(button.dataset.action==='reset'){yaw=.18;pitch=-.16;showStep(0);cameraTarget=null;draw();return;}
        const direction=button.dataset.action==='next'?1:-1;
        if(selected)beginRecord(Math.max(0,Math.min(byId.get(selected).children.length-1,activeRecord===null?0:activeRecord+direction)),true);
        else showStep(Math.max(0,Math.min(steps.length-1,index+direction)));
      });
    });
    stage.addEventListener('pointerdown',event=>{if(selected||event.target.closest('a,button')||(event.pointerType==='mouse'&&event.button!==0))return;stopNarration();cameraTarget=null;pointer={id:event.pointerId,x:event.clientX,y:event.clientY};stage.setPointerCapture(event.pointerId);stage.classList.add('is-dragging');});
    stage.addEventListener('pointermove',event=>{if(!pointer||pointer.id!==event.pointerId)return;yaw+=(event.clientX-pointer.x)*.006;pitch=Math.max(-1,Math.min(1,pitch+(event.clientY-pointer.y)*.004));pointer.x=event.clientX;pointer.y=event.clientY;draw();});
    const release=()=>{pointer=null;stage.classList.remove('is-dragging');};
    stage.addEventListener('pointerup',release);stage.addEventListener('pointercancel',release);stage.addEventListener('lostpointercapture',release);
    reduced.addEventListener('change',()=>{stopNarration();settleTransition();cameraTarget=null;if(beat&&!beat.arrived){beat.age=beat.travel+700;presentRecord();}syncMotion();});
    document.addEventListener('visibilitychange',()=>{if(document.hidden){stopNarration();cameraTarget=null;settleTransition();}syncMotion();});
    if(window.IntersectionObserver)new IntersectionObserver(entries=>{visible=entries[0].isIntersecting;if(!visible){stopNarration();cameraTarget=null;settleTransition();}syncMotion();}).observe(root);
    if(window.ResizeObserver)new ResizeObserver(()=>{measureLens();draw();}).observe(stage);
    window.addEventListener('pagehide',()=>{visible=false;stopNarration();cameraTarget=null;settleTransition();syncMotion();});
    window.addEventListener('pageshow',()=>{const box=root.getBoundingClientRect();visible=box.bottom>0&&box.top<window.innerHeight;syncMotion();});
    root.classList.add('is-enhanced');chooseCase(examples[0]);syncMotion();
    if(immersive&&new URLSearchParams(location.search).get('view')==='graph')toggleFocus();
  });
})();

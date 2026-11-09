const paperDgm = 'https://arxiv.org/abs/2505.22954';
const paperStop = 'https://arxiv.org/abs/2310.02304';
const commit = 'https://github.com/aster-ai-lab/aster/blob/ba17a7ca69bb39bd6a98dce14377777951fa9119/';
const points = [[-.68,.5,.2],[-.62,-.45,.42],[0,.03,.95],[.58,.52,.3],[.64,-.32,.42],[.1,-.73,.3],[-.2,.75,-.56],[.8,.08,-.5]];
const detail = (label, text) => ({ label, detail:text });
function makeCase(id,title,type,scope,source,rows,phases) {
  const nodes=rows.map(([key,label,kind,owner,text,children,url],i)=>({id:key,label,kind,owner,detail:text,children,source:url||source,point:points[i]}));
  const steps=phases.map(([title,focus,active,body,result])=>({title,focus,active,body,result}));
  return {id,title,type,scope,source,nodes,steps};
}
export const realCases = [
  makeCase('dgm','DGM · 编码 Agent 自修改','文献流程重建 · 2025',
    '基于 DGM 原论文第 3–4 节、图 3 与附录 C。阶段顺序表达算法流程，连线不是完整实验谱系；改动和结果来自作者报告，未在 Aster 中复现。',paperDgm,[
    ['seed','初始编码 Agent','policy','原论文 §3、附录 A.1','初始 Agent 以冻结的基础模型调用工具完成编码任务；演化对象是 Agent 的 Python 实现。',[detail('改进对象','工具实现与工作流程，不是本实验中的模型权重。'),detail('起始能力','读取、修改与执行代码，为后续自修改提供基础。')]],
    ['archive','Agent 档案库','evidence','原论文 §3','保存不同 Agent 版本，允许从不止一个候选继续探索。',[detail('为什么保留分支','即时得分较低的版本也可能成为后续改进的跳板。'),detail('图 3','作者给出实际谱系；本图是用于阅读的关系重建。')]],
    ['parent','选择父 Agent','session','原论文 §3、附录 A.2','从档案库选择父 Agent，再由它修改自身实现。',[detail('递归位置','新发现的 Agent 后续也能承担自修改。'),detail('与固定元 Agent 的区别','始终用初始 Agent 改写其他 Agent，是论文中的另一条对照设置。')]],
    ['edit','文件工具变更','component','原论文 §4.4、附录 C','作者报告：文件查看细化到行，编辑支持字符串替换。',[detail('改动前','初始工具以整文件查看或替换为主。'),detail('改动后','提供更细粒度的查看和定点编辑。'),detail('Aster 接入映射','应用设计建议：通过会话注册工具实现，tools 视图提供目录；执行权限仍由 Core 决定。')]],
    ['review','多候选与评审','candidate','原论文 §4.4、附录 C','作者还报告生成、比较多个解法，并在新尝试中保留此前失败的信息。',[detail('实际报告','多候选选择与尝试历史属于作者列出的工作流改动。'),detail('不是单一路线','这些改动不表示每个 Agent 都按同一顺序获得全部能力。'),detail('Aster 接入映射','设计建议：宿主组件组织候选与比较，结果写入同一事实日志。')],paperDgm],
    ['eval','编码任务评估','evidence','原论文 §4.2–4.4','变体接受分阶段编码评估；预算不同的评估子集需要分别标明。',[detail('评估用途','确认代码编辑能力，并估计下游编码表现。'),detail('不能混用的口径','论文的子集搜索结果与完整 Polyglot 结果有各自范围。')]],
    ['lineage','保留演进谱系','candidate','原论文图 3','论文展示实际谱系中的中间跳板；更好版本并非总沿着即时最优分支出现。',[detail('节点 24','图 3 将其描述为带来许多后续创新的关键节点。'),detail('实证范围','这是该次实验的观察，不是无限改进的保证。')]],
    ['result','论文结果与边界','decision','论文 §4 · 非 Aster 结果','作者报告 SWE-bench 评估中从 20.0% 到 50.0%，完整 Polyglot 从 14.2% 到 30.7%。',[detail('来源','论文 §4 的 Results；SWE-bench 与完整 Polyglot 口径分别标明。'),detail('使用范围','这些数字不属于 Aster；当前页面没有重新运行相关基准。'),detail('后续应用','移植某个工具或流程仍需在自身任务集上独立验证。')],paperDgm],
  ],[
    ['从初始 Agent 出发','seed',['seed','archive'],'先建立能读写与执行代码的初始 Agent，基础模型保持冻结。','观察对象：编码 Agent 的实现。'],
    ['从档案选择父代','parent',['archive','parent','seed'],'选择父 Agent，让已有版本参与产生后继。','档案提供多条探索路线。'],
    ['钻取实际工具改动','edit',['parent','edit'],'点击文件工具，查看作者报告的整文件操作到细粒度操作的改变。','来源定位：论文 §4.4 与附录 C。'],
    ['查看工作流演进','review',['parent','review','edit'],'打开多候选与评审，查看生成多个方案、比较选择与尝试历史。','来源是作者报告，不补造单次运行日志。'],
    ['评估后保留谱系','eval',['edit','review','eval','archive','lineage'],'结合编码评估与档案，保留能够继续探索的版本。','分阶段评估的结果口径必须分开。'],
    ['核对结果及局限','result',['result','lineage','archive'],'查看报告的端点结果，再进入下一轮父代选择。','不画未经来源提供的中间性能曲线。'],
  ]),
  makeCase('stop','STOP · 改进器改进自身','文献流程重建 · COLM 2024',
    '基于 STOP 论文图 2–3、Algorithm 1、策略分析与官方代码仓库。这里重建算法关系，不把不同实验产生的搜索策略拼成一条伪造的部署历史。',paperStop,[
    ['seed','Seed improver','policy','STOP 图 2','种子改进器多次请求模型生成程序候选，再按效用函数选择。',[detail('输入','初始程序、效用函数和语言模型。'),detail('输出','在本次候选中按效用选择的程序。')]],
    ['utility','下游效用','evidence','STOP Algorithm 1','效用衡量改进后的程序在下游任务上的表现。',[detail('任务层','先运行改进器得到程序，再评价这个程序。'),detail('元层','改进器的价值来自它改善下游程序的能力。')]],
    ['self','以自身为输入','session','STOP 图 3、Algorithm 1','将改进器自身代码交给改进器，以元效用评价产生的新改进器。',[detail('递归步骤','上一代改进器参与生成下一代改进器。'),detail('模型边界','论文未修改基础语言模型本身。')]],
    ['search','搜索策略分支','component','STOP 论文策略分析','作者分析了模型提出的束搜索、遗传算法和模拟退火等策略。',[detail('具体变化','改的是怎样生成、搜索与选择代码候选。'),detail('证据边界','这是论文分析的一组策略，不是三次已知顺序的上线。'),detail('Aster 接入映射','设计建议：由宿主组件或完整 Assembler 承载候选搜索，保持 Core 的权限和完成边界。')]],
    ['evaluate','评估新改进器','evidence','STOP Algorithm 1','新改进器再次优化下游程序，用任务效用汇总其表现。',[detail('比较对象','比较改进器对任务的效果，而非只比较提示文本长度。'),detail('实验范围','论文是受限范围的代码生成自改进研究。')]],
    ['next','下一代 improver','candidate','STOP 论文策略分析','改进后的改进器继续参与后续改进；语言模型保持不变。',[detail('实际研究含义','递归的是模型外部的脚手架代码。'),detail('应用边界','移植到 Aster 需要独立实现与验证，不能把研究结果当成本产品表现。')]],
  ],[
    ['查看种子改进器','seed',['seed','utility'],'点击 Seed improver，查看候选生成与效用选择的输入输出。','来源：论文图 2。'],
    ['进入自应用步骤','self',['seed','self','utility'],'把改进器自己变成待优化的程序。','来源：论文图 3 与 Algorithm 1。'],
    ['钻取搜索策略','search',['self','search'],'查看作者报告的不同搜索程序思路。','不同策略是分支，不能伪造为同一条历史。'],
    ['评价下一代','evaluate',['utility','search','evaluate'],'用下游任务效用评价改进器，而非让它自己宣称更好。','元效用关联真实下游结果。'],
    ['继续递归','next',['evaluate','next','self'],'下一代改进器再参与生成后继。','不等同于基础模型权重自训练。'],
  ]),
  makeCase('aster','Aster · 长任务预算修复','仓库工程记录 · 2026-10-20',
    '对应提交 ba17a7ca69bb39bd6a98dce14377777951fa9119 及其保留的验收记录。这是人工组织的 Meta Harness 工程演进，不是 Aster 已自主完成 RSI 的证明。',commit+'docs/evidence/code/2026-10-20-acp-runtime-landing/OUTCOME.md',[
    ['failure','长任务被截断','failure','提交说明与工程证据','交互式 ACP 的隐式工具或状态转换次数上限影响长任务。SDK 的默认界限与显式配置仍需保留。',[detail('修复目标','移除交互路径的隐式次数天花板，保留用户与父级明确设置。'),detail('范围','工程案例来自该提交，不能代表所有长任务都已验证。')]],
    ['limit','区分 None 与 0','component','该提交 · Effect 配置','用类型化的可选上限区分“无计数上限”和“零次工具额度”。',[detail('改动前的风险','用数值投影表达无上限，会混淆零额度含义。'),detail('改动后','执行路径读取真实 Option 限制，旧数值接口仍保留兼容语义。')],commit+'crates/effect/src/coding.rs'],
    ['compose','组合准入保留约束','component','该提交 · Core Meta Harness','配方与宿主注册组件的准入路径保留真实可选限制；显式配方限制继续生效。',[detail('具体接口','admit_from_compose_with_registry 与 optional_budget 的接入。'),detail('不能删除的边界','权限、验证门禁、取消与重复调用保护不随计数上限取消。')],commit+'crates/code/core/src/meta_harness.rs'],
    ['tests','有限脚本回归','evidence','已提交的测试源','测试包含 128 次工具调用的受控脚本、64 次后取消、显式零额度与有限步数等情况。',[detail('它证明什么','在确定脚本与测试边界内核对计数和取消合同。'),detail('不证明什么','不能据此推算真实模型任务速度、费用或成功率。')],commit+'crates/effect/tests/unbounded_counts.rs'],
    ['record','历史验收记录','decision','2026-10-20 验收记录','记录报告该次隔离检查共 686 项通过、2 项已有忽略；同时明确全工作区检查仍有失败。',[detail('记录范围','历史验收对应当时冻结的源与命令。'),detail('当前状态','本网页没有重新执行那些 Rust 检查，历史通过不能冒充当前 CI。')]],
    ['boundary','候选之外的门禁','guard','Core 合同与该次修复范围','可替换执行策略与固定治理边界分开。计数取消不产生权限，也不把未决副作用变成成功。',[detail('工程演进','这是 Meta Harness 可以承载的实际策略修复。'),detail('RSI 映射','未来宿主可以把类似差异作为候选评估；搜索、接纳与回滚仍需宿主实现。')],commit+'crates/code/core/src/meta_harness.rs'],
  ],[
    ['从真实问题进入','failure',['failure','limit'],'点击失败节点，查看长任务次数上限的实际修复范围。','来源：提交 ba17a7ca6。'],
    ['钻取预算语义','limit',['limit','compose'],'核对 None、零额度和显式限制的区别。','保持 SDK 兼容语义。'],
    ['进入装配准入','compose',['compose','boundary'],'查看组合路径如何继承限制，哪些门禁不受影响。','装配可变，权限与完成边界保留。'],
    ['查看回归测试','tests',['tests','limit','boundary'],'进入具体测试源，查看长脚本、取消与显式上限。','测试场景不是模型任务收益。'],
    ['追溯历史验收','record',['record','tests','boundary'],'打开该次记录，阅读通过项、忽略项和已知限制。','历史证据与当前工作树状态分开。'],
  ]),
];


const group = (label,kind,...records) => ({label,kind,records});
const extraNode = (id,label,kind,owner,text,children,source) => ({id,label,kind,owner,detail:text,children,source});
const dgm=realCases[0],stop=realCases[1],aster=realCases[2];
Object.assign(dgm.nodes.find(n=>n.id==='eval'),{
  label:'候选仍能编译与编辑吗？',
  detail:'DGM 先评估候选，再检查其能否编译并继续编辑代码。有效候选加入档案；低分本身不是排除条件。',
});
dgm.nodes.find(n=>n.id==='eval').children.push(detail('准入条件','论文 §3 与附录 A.4：保留可以编译并保持代码编辑能力的候选，其他候选不入档。'));
dgm.nodes.push(extraNode('invalid','无效候选 · 不入档','failure','原论文 §3、附录 A.4 · 机制分支','编译失败或失去代码编辑能力的候选不会加入档案。这是论文算法的条件结果，不声称此处展示了某一次失败运行。',[
  detail('判断依据','检查编译与代码编辑能力，不用“分数较低”代替有效性检查。'),detail('分支结果','该候选不入档，原有档案继续用于下一轮父代选择。'),
],paperDgm));
Object.assign(dgm.steps[4],{title:'判断是否可以继续演化',body:'候选经编码评估后接受有效性检查。能编译并继续编辑代码则入档；否则排除。低分但有效的分支仍可作为后续跳板。',result:'有效性决定入档；即时分数不等于全部探索价值。'});
for(const [id,label,text]of [
  ['beam','束搜索','扩展并保留多个候选，继续沿若干候选路线搜索。'],
  ['genetic','遗传搜索','用候选群体与变异等操作探索程序空间。'],
  ['anneal','模拟退火','以退火式搜索探索候选程序。'],
])stop.nodes.push(extraNode(id,label,'candidate','STOP 图 1 与策略分析 · 论文汇总',text,[
  detail('改动方向',text),detail('记录边界','这是作者报告的策略类型；本图不声称它与另外两类策略在同一轮同时执行，也不补造该分支得分。'),
],paperStop));
stop.nodes.find(n=>n.id==='search').label='探索哪种搜索策略？';
stop.nodes.find(n=>n.id==='evaluate').label='哪个候选的效用最高？';
stop.nodes.find(n=>n.id==='evaluate').detail='种子改进器按效用从本次生成的候选中选优。三类策略节点是论文汇总，不表示同一次运行的并行执行日志。';
for(const [id,label,text]of [
  ['unbounded','None · 无计数上限','显式无计数上限的交互路径可以继续执行长工具脚本；权限、取消和完成检查仍然生效。'],
  ['zero','Some(0) · 零次工具','显式零工具额度会阻止工具执行，不能被解释成无上限。'],
  ['finite','Some(n) · 正数上限','明确设置的有限额度仍约束工具次数，达到上限后停止相应调用。'],
])aster.nodes.push(extraNode(id,label,'guard','已提交的额度合同 · 配置分支',text,[
  detail('配置语义',text),detail('验证依据','对应同一提交中的 unbounded_counts.rs 回归场景；不把该配置分支当成某次真实模型运行结果。'),
],commit+'crates/effect/tests/unbounded_counts.rs'));
aster.nodes.find(n=>n.id==='limit').label='工具额度是哪种配置？';
aster.steps=[aster.steps[0],aster.steps[2],aster.steps[1],aster.steps[3],aster.steps[4]];
const layouts={
  dgm:{seed:[-.94,-.25,.2],archive:[-.58,.65,.35],parent:[-.5,.1,.65],edit:[-.08,.6,.75],review:[-.12,-.5,.75],eval:[.4,.1,.7],invalid:[.8,-.5,.2],lineage:[.72,.65,.1],result:[.94,.03,-.15]},
  stop:{seed:[-.95,-.2,.1],utility:[-.6,-.55,.4],self:[-.55,.45,.55],search:[-.15,.08,.85],beam:[.22,.72,.45],genetic:[.3,.1,.7],anneal:[.25,-.6,.5],evaluate:[.68,.12,.35],next:[.96,.5,.02]},
  aster:{failure:[-.95,-.1,.1],compose:[-.57,.4,.45],limit:[-.15,.1,.8],unbounded:[.25,.72,.4],zero:[.3,.1,.55],finite:[.25,-.6,.5],tests:[.7,.1,.35],record:[.96,.5,.02],boundary:[-.55,-.55,.35]},
};
const scenarios={
  dgm:{title:'编码 Agent 怎样改进自己的工具与工作流？',goal:'保留有效自修改，再从档案继续探索。',baseline:'初始 Agent 使用整文件操作；基础模型冻结。',decision:'候选能编译并继续编辑代码吗？',outcome:'有效候选入档；无效候选排除。'},
  stop:{title:'让代码改进器改进自己的搜索程序',goal:'用下游任务效用比较候选改进器。',baseline:'种子程序批量生成候选，再按效用选优。',decision:'哪些候选在给定效用下表现更好？',outcome:'本批选出的改进器参与下一轮递归。'},
  aster:{title:'修复长任务被隐式次数上限截断的问题',goal:'移除交互路径的隐式天花板，保留显式约束。',baseline:'无上限、零额度与有限上限需要分别处理。',decision:'配置是 None、Some(0) 还是 Some(n)，n > 0？',outcome:'三个配置分支各按其合同执行，并接受回归验证。'},
};
const roles={
  dgm:{seed:'scenario',archive:'evidence',parent:'action',edit:'action',review:'action',eval:'decision',invalid:'outcome',lineage:'outcome',result:'outcome'},
  stop:{seed:'scenario',utility:'evidence',self:'action',search:'decision',beam:'branch',genetic:'branch',anneal:'branch',evaluate:'decision',next:'outcome'},
  aster:{failure:'scenario',compose:'action',limit:'decision',unbounded:'outcome',zero:'outcome',finite:'outcome',tests:'evidence',record:'outcome',boundary:'evidence'},
};
const edge = (from,to,label,kind='sequence',visible=false) => ({from,to,label,kind,visible});
const flows={
  dgm:[edge('seed','archive','保存基线'),edge('archive','parent','选择父代'),edge('parent','edit','工具改动','option',true),edge('parent','review','工作流改动','option',true),edge('edit','eval','评估候选'),edge('review','eval','评估候选'),edge('eval','archive','是 · 入档','feedback',true),edge('eval','invalid','否 · 不入档','reject',true),edge('archive','lineage','保留探索分支'),edge('lineage','result','作者报告','evidence',true)],
  stop:[edge('seed','utility','给定效用','evidence'),edge('seed','self','以自身为输入'),edge('self','search','提出搜索程序'),edge('search','beam','策略 A','option',true),edge('search','genetic','策略 B','option',true),edge('search','anneal','策略 C','option',true),edge('beam','evaluate','比较候选'),edge('genetic','evaluate','比较候选'),edge('anneal','evaluate','比较候选'),edge('utility','evaluate','评分依据','evidence'),edge('evaluate','next','本批择优','condition',true),edge('next','self','递归下一轮','feedback',true)],
  aster:[edge('failure','compose','修复组合准入'),edge('compose','limit','保留类型化额度'),edge('limit','unbounded','None','condition',true),edge('limit','zero','Some(0)','condition',true),edge('limit','finite','n > 0','condition',true),edge('unbounded','tests','长脚本回归'),edge('zero','tests','零额度回归'),edge('finite','tests','有限上限回归'),edge('boundary','compose','固定治理约束','evidence'),edge('tests','record','历史隔离验收','evidence',true)],
};
const roleLabels={scenario:'场景',action:'改动',decision:'决策',branch:'方案',outcome:'结果',evidence:'依据'};
for(const item of realCases){
  item.scenario=scenarios[item.id];item.flow=flows[item.id];item.links=item.flow.map(({from,to})=>[from,to]);
  for(const node of item.nodes){node.point=layouts[item.id][node.id];node.role=roles[item.id][node.id];node.roleLabel=roleLabels[node.role];if(!node.point||!node.roleLabel)throw new Error(`Missing scene role or position: ${item.id}/${node.id}`);}
}

export function attachGroups(nodes, groups) {
  for (const node of nodes) {
    node.groups=groups[node.id];
    if(!node.groups?.length)throw new Error(`Missing relationship groups: ${node.id}`);
    const records=node.groups.flatMap(group=>group.records);
    if(records.length!==node.children.length||new Set(records).size!==records.length||records.some(i=>!Number.isInteger(i)||i<0||i>=node.children.length))throw new Error(`Relationship groups must cover each record once: ${node.id}`);
  }
}
const caseGroups = {
  dgm:{
    seed:[group('Agent 实现','component',0,1)],
    archive:[group('分支探索','candidate',0),group('谱系证据','evidence',1)],
    parent:[group('自修改','candidate',0),group('实验对照','evidence',1)],
    edit:[group('工具变化','component',0,1),group('Aster 接入','candidate',2)],
    review:[group('工作流变化','component',0),group('证据边界','guard',1),group('Aster 接入','candidate',2)],
    eval:[group('下游评估','evidence',0),group('结果口径','guard',1),group('准入判断','decision',2)],
    invalid:[group('有效性检查','guard',0),group('分支结果','failure',1)],
    lineage:[group('实际谱系','candidate',0),group('研究边界','guard',1)],
    result:[group('作者结果','evidence',0),group('适用边界','guard',1,2)],
  },
  stop:{
    seed:[group('改进器合同','component',0,1)],
    utility:[group('程序效用','evidence',0),group('改进器效用','evidence',1)],
    self:[group('递归步骤','candidate',0),group('模型边界','guard',1)],
    search:[group('搜索程序','component',0),group('研究边界','guard',1),group('Aster 接入','candidate',2)],
    beam:[group('搜索程序','component',0),group('证据边界','guard',1)],
    genetic:[group('搜索程序','component',0),group('证据边界','guard',1)],
    anneal:[group('搜索程序','component',0),group('证据边界','guard',1)],
    evaluate:[group('效用比较','evidence',0),group('实验范围','guard',1)],
    next:[group('改进对象','component',0),group('应用边界','guard',1)],
  },
  aster:{
    failure:[group('工程问题','failure',0),group('修复范围','guard',1)],
    limit:[group('修改前后','component',0,1)],
    compose:[group('装配准入','component',0),group('固定边界','guard',1)],
    unbounded:[group('额度语义','guard',0),group('验证依据','evidence',1)],
    zero:[group('额度语义','guard',0),group('验证依据','evidence',1)],
    finite:[group('额度语义','guard',0),group('验证依据','evidence',1)],
    tests:[group('测试合同','evidence',0),group('证明范围','guard',1)],
    record:[group('历史证据','evidence',0),group('当前状态','guard',1)],
    boundary:[group('工程记录','evidence',0),group('RSI 映射','candidate',1)],
  },
};
const routes = {
  dgm:[['seed'],['archive','parent'],['parent','edit'],['parent','review'],['parent','edit','eval'],['archive','lineage','result']],
  stop:[['seed'],['seed','self'],['self','search'],['search','beam','evaluate'],['evaluate','next']],
  aster:[['failure'],['failure','compose'],['compose','limit'],['limit','unbounded','tests'],['tests','record']],
};
for(const item of realCases) {
  attachGroups(item.nodes,caseGroups[item.id]);
  item.steps.forEach((step,i)=>{
    step.route=routes[item.id][i];
    if(step.route.at(-1)!==step.focus||step.route.some(id=>!item.nodes.some(node=>node.id===id))||step.route.slice(1).some((id,j)=>!item.links.some(([a,b])=>a===step.route[j]&&b===id)))throw new Error(`Invalid replay route: ${item.id}/${i}`);
  });
}

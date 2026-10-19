本轮对应参考 data.json 中四个校核流程的本地行为。inputs/ 中均为合成文本。只使用本地材料和用户明确选择的校核项；不调用外部系统，不执行原导出流程的 Code/Http 节点，不声称检索过机构、人员或法规。

请输出四个独立文件 W01.json 至 W04.json，并检查它们。各 finding 必须含 errortext（原文中精确连续引文）、checkresult（实际修改建议，不能与原文相同）、type（basic/official/security）、reason。不得把“无错误”作为 finding，不为凑数润色正确文本，不编造引文、法规或文件内容。

W01 混合流程：输出对象含 ambiguous、selected、mixed 三个场景。
- ambiguous 场景：候选文件为 inputs/a.txt 和 inputs/b.txt。用户只说“检查这个文件”，未指明文件。输出 selectedFiles:[]、findings:[] 并用 clarification 说明需要用户明确选择；不能默认最新。
- selected 场景：用户明确选择 inputs/a.txt，仅启用基础校核。用户的本地原样保留白名单为“按排”。输出 selectedFiles、findings；仅检查显然的错字/语法/标点，不做公文或安全分析，不对白名单字符串提出修改。
- mixed 场景：用户明确选择 inputs/notice.txt，启用基础、公文和安全三类校核。先由语义判断是否为公文，输出 documentType、selectedFiles、findings。合并三类结果，删除重复 finding，保留全部原文引文和问题类型。

W02 公文流程：检查 inputs/notice.txt，用户只选择“公文格式”和“行文规范”，不做基础错字或安全校核。输出 selectedChecks:["公文格式","行文规范"]、findings。每条另含 checkItem，必须属于已选项；重点核对文种与行文意图、日期与结构格式。引用原句，并说明修改原因。

W03 安全流程：输出 relevant 和 irrelevant 两个场景。relevant 检查 inputs/security.txt，先判断适用性，输出 applicable:true、findings，只检查方案本身的安全与逻辑问题。irrelevant 检查 inputs/park.txt，输出 applicable:false、findings:[]。不能把普通生活短文硬套安全方案标准，不以错别字替代高风险方案问题。

W04 基础流程：输出 incorrect 和 correct 两个场景。incorrect 检查 inputs/a.txt，不设置白名单，只修显然错字/语法/标点；correct 检查 inputs/b.txt，正确文本应 findings:[]。不要做风格改写、公文或安全校核。

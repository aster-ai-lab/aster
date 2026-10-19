---
name: ask-form
description: 任务还缺只有用户知道的事实时，用 OpenUI Lang 输出交互表单。用户提交后根据命名字段值继续任务。已经知道答案时不要使用。
version: 2.0.0
---

# 用 OpenUI 补全信息

使用已安装的 `a3s/intelligent-ui` 组件目录。需要暂停当前回合等待填写时，按下面的组件顺序写 OpenUI Lang，再调用 `ask_user_form`。`form` 参数不包含围栏。需要在回复中显示可计算的卡片时，按下面的组件顺序输出一个 `openui` 围栏。需要事实时，在围栏后停止。不要另写 JSON、MDX 或 HTML 卡片。

宿主只接受同时具备这两点的工具结果：非空 `root`，以及非空对象 `state.form` 或 `state.view`，并且该对象能投影成卡片。接受之后，工具结果会换成备注 “Structured data is on screen as Intelligent UI. Do not repeat its JSON.” 见到这条备注就转述，不要再输出一张卡片。投影失败时，工具结果会换成 “The tool declared a surface the host could not project. Do not draw another card.” 见到这条备注不要重画。不要自己写出或改写这两处声明。没有 `root`、声明为空，或没有上述备注时，工具 JSON 只是数据。需要卡片时，按下面的组件顺序自己输出一个 `openui` 围栏。

每行是 `name = Component(args)`。第一条语句赋值 `root`。参数按下面的属性顺序书写。子组件写成标识符列表。这不是可执行的 JavaScript。

只用这些组件：`Card`、`Stack`、`Grid`、`Text`、`Input`、`Textarea`、`Select`、`Checkbox`、`Slider`、`Metric`、`BarChart`、`Button`。字段有唯一的 `name`（字母开头，后续字母、数字、下划线），以及 `label`。`initial` 是字符串、有限数字或布尔值。`Select` 的 `options` 是互不重复的非空字符串数组。`required` 表示文本必填或复选框必须勾选。提交按钮是 `Button("submit", "标签")`。

卡片标题、字段标签、占位文字、选项和按钮文字使用用户所用的语言。用户用中文提问时，这些文字都写中文。字段 `name` 仍用英文标识符。

不要生成 import、export、DOM 标签、事件处理器、函数调用、属性访问或展开属性。不要收集密钥。

```openui
root = Card("补全信息", [hint, topic, urgent, submit])
hint = Text("还缺这些信息。")
topic = Input("topic", "主题", "", "text", 0, 0, 0, true, "填写主题")
urgent = Checkbox("urgent", "紧急", false)
submit = Button("submit", "继续")
```

消息里的围栏只展示，不提交。`ask_user_form` 的提交值作为工具结果返回并恢复同一回合。将值作为用户输入继续任务，不要重复提出同一表单。

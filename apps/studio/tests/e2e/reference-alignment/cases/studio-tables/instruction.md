以下四项任务来自 features.xlsx / Sheet1!C2:C5。附件位于工作区的 inputs/；它们全部是合成测试数据。请分别完成，不覆盖原始附件，所有结果保存到当前工作区。

F01：帮我把这些报名表合并成一张汇总表，统一格式并去重。
输入为 inputs/registration-a.xlsx 和 inputs/registration-b.xlsx。按邮箱去重，邮箱忽略大小写；邮箱为空的记录各自保留，不能按姓名合并。日期统一为 YYYY-MM-DD，保留来源文件名。输出 F01.csv，表头严格为 name,email,department,date,sources。

F02：把我团队成员的周报合并整理成一份，按项目维度汇总进展。
输入为 inputs/weekly.md。输出 F02.json，顶层为 projects 数组；每个项目有 name、updates、risks、nextActions、conflicts 数组。各数组条目是含 text、source、quote 的对象：source 是成员姓名，quote 是周报中逐字连续的原句。相互矛盾的陈述放入 conflicts，不替团队决定谁正确。

F03：帮我从这些发票文件里提取日期、金额、税号等信息，整理成表格。
输入为 inputs/invoices.pdf，共三页。按发票号去重，保留所有来源页码；未提供的税号留空，不把零金额当缺失。输出 F03.csv，表头严格为 invoice_id,date,amount,tax_id,source_pages。

F04：帮我清洗这份数据：去重、格式化日期列、统一文本大小写。
输入为 inputs/dirty-data.xlsx。sku 统一大写；有效日期统一 YYYY-MM-DD；按清洗后的全部数据列去重。非法日期保留原字符串并标记 invalid_date；空日期留空并标记 missing_date；其他行标记 ok。保留数值零与空值的区别。输出 F04.csv，表头严格为 id,date,sku,value,status。

CSV 使用 UTF-8（可带 BOM）。完成后检查输出数据与原材料一致，并说明无法完成的项目。

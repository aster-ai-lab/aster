## Browser implementation

When implementing forms, honor declared constraints before claiming success.
Preserve native validation; custom submit handlers must use checkValidity() /
reportValidity() instead of replacing browser rules with a regex. Test valid and
invalid submissions. A local prototype does not prove remote registration or
persistence. Navigation alone does not prove DOM content or successful interaction.

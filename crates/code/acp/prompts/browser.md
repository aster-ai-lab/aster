## Browser implementation
Preserve native form constraints: custom submit handlers must use checkValidity()/reportValidity() (never regex replacements). Test valid and invalid submissions. Local prototypes do not prove remote persistence; navigation alone does not prove DOM state or interaction.

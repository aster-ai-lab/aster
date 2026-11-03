# Research

The user chose compact Aster geometry with Arc material and interaction.

- Existing Arc source and CSS are already under `packages/ui/src/components/arc`; reuse them without downloading another component library.
- Official [motion](https://uiarc.dev/docs/motion) specifies zero-overshoot springs, immediate keyboard navigation, interruption and reduced-motion branches.
- Official [theming](https://uiarc.dev/docs/theming) uses semantic surface/text/border roles. Aster's DESIGN.md and compact-size choice own the final dimensions and palette.
- Official [dropdown menu](https://uiarc.dev/components/dropdown-menu) uses one gliding highlight. The existing ComposerPopupMotion already measures real item/containing-block geometry; move this behavior to the shared owner rather than reimplementing it per surface.
- Native Button uses Arc while link/asChild/custom-loader paths use separate utilities. Preserve their semantics while sharing styling.
- SearchSelect has a second per-row highlight implementation and an opt-in composer flag. Reuse the common measured plate while retaining its public flag and choice/search logic.
- Existing popup presence and motion CSS have concurrent edits. Keep those changes and separate the current task's patch.

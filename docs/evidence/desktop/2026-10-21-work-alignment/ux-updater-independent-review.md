# Read-only updater renderer compatibility review

Reviewed the current native return contract and its renderer callers, without running or modifying the update mechanism.

`app_updater_install` now returns `Err(code)` for failed install/relaunch instead of resolving. `src/renderer/shell/desktopMainBridge.js` converts a rejected native invocation to an error envelope; `src/renderer/ipc/ipcApi.ts:18` reconstructs and throws it. Both `UpdateDialogPopup.handleInstall` and `AboutSettings.onRestart` catch that error, release their installing state and show the existing localized `settings.about.updateError`. The popup does not resolve a failed install as success. The successful native path still exits the process.

No concrete compatibility defect found in this path. The five typed native reasons currently share the generic localized renderer error message, as scoped by the parent; this review does not claim reason-specific remediation UI. Native signature/product/integrity/install/relaunch execution and actual application restart remain separate verification obligations. No updater tests were duplicated, and no native or user installation was changed by this review.

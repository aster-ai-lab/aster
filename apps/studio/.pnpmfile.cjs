// browserslist and wherearewe are patched to stop loading these packages.
// Deleting the edges here is what keeps them out of the lockfile: a patch
// changes the files pnpm installs, not the manifest it resolves.

function readPackage(pkg) {
  if (pkg.name === 'browserslist' && pkg.dependencies) {
    delete pkg.dependencies['electron-to-chromium']
  }
  if (pkg.name === 'wherearewe' && pkg.dependencies) {
    delete pkg.dependencies['is-electron']
  }
  return pkg
}

module.exports = {
  hooks: {
    readPackage
  }
}

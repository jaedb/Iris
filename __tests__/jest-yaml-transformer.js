const transformer = require("jest-transform-yaml").default

const newTransformer = {
  ...transformer,
  process: function (...params) {
    return {
      code: transformer?.process(...params),
      map: null,
    }
  },
}

module.exports = newTransformer;
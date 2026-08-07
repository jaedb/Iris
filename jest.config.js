module.exports = {
  verbose: true,
  collectCoverage: true,
  testEnvironment: 'jsdom',
  testMatch: [
    "<rootDir>/__tests__/**/*.test.js?(x)",
  ],
  // moduleDirectories: [
  //   "node_modules",
  //   "src",
  // ],
  // moduleFileExtensions: [
  //   "js",
  //   "jsx",
  //   "yaml",
  // ],
  moduleNameMapper: {
    /*"\\.(jpg|jpeg|png|gif|eot|otf|webp|svg|ttf|woff|woff2|mp4|webm|wav|mp3|m4a|aac|oga|yml|yaml)$": "<rootDir>/__mocks__/fileMock.js",*/
    "^.+\\.(css|scss|sass|less)$": "identity-obj-proxy",
  },
  transform: {
    "^.+\\.(j|t)sx?$": "babel-jest",
    "^.+\\.ya?ml$": "<rootDir>/__tests__/jest-yaml-transformer.js",
  },
  transformIgnorePatterns: [
    "node_modules/(?!react-dnd|core-dnd|@react-dnd|dnd-core|react-dnd-html5-backend|react-redux)"
  ],
  coverageReporters: [
    "lcov",
  ],
  collectCoverageFrom: [
    "src/**/*.{js,jsx,ts,tsx}",
  ],
  globals: {
    window: {
      location: {
        hostname: 'localhost',
        port: 6680,
        protocol: 'http',
      },
    },
  },
};

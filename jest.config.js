module.exports = {
  verbose: true,
  collectCoverage: true,
  testMatch: [
    "<rootDir>/__tests__/**/*.test.js?(x)",
  ],
  moduleDirectories: [
    "node_modules",
    "src",
  ],
  moduleFileExtensions: [
    "js",
    "jsx",
    "yaml",
  ],
  moduleNameMapper: {
    /*"\\.(jpg|jpeg|png|gif|eot|otf|webp|svg|ttf|woff|woff2|mp4|webm|wav|mp3|m4a|aac|oga|yml|yaml)$": "<rootDir>/__mocks__/fileMock.js",*/
    "^.+\\.(css|scss|sass|less)$": "identity-obj-proxy",
  },
  transform: {
    "\\.yaml$": "yaml-jest",
    "^.+\\.jsx$": "babel-jest",
    "^.+\\.js$": "babel-jest",
    "^.+\\.ts$": "babel-jest",
    "^.+\\.tsx$": "babel-jest",
  },
  coverageReporters: [
    "lcov",
  ],
  collectCoverageFrom: [
    "src/**/*.{js,jsx,ts,tsx}",
  ],
  setupFilesAfterEnv: [
    "./__tests__/setup.js"
  ],
};

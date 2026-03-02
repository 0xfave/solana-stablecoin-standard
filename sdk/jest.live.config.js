module.exports = {
  preset: "ts-jest",
  testEnvironment: "node",
  testMatch: ["**/__tests__/live.test.ts"],
  moduleFileExtensions: ["ts", "js", "json"],
  testTimeout: 120000,
};

const { pow } = require("./lib");

test("exp", () => {
  expect(pow(2, 3)).toBe(8);
});

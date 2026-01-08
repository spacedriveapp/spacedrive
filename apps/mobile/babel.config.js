module.exports = (api) => {
  api.cache(true);
  return {
    presets: [["babel-preset-expo", { jsxImportSource: "nativewind" }]],
    plugins: [
      ["@babel/plugin-transform-runtime", { helpers: true }],
      "react-native-reanimated/plugin",
    ],
  };
};

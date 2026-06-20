"use strict";

/* eslint-disable */
const packageVersion = require("./package.json").version;
const webpack = require("webpack");
const HtmlWebpackPlugin = require("html-webpack-plugin");
const TerserWebpackPlugin = require("terser-webpack-plugin");

const isDevServer = process.argv.includes("serve");
/* eslint-enable */

const commonConfig = {
  target: ["web", "es2017"],
  mode: isDevServer ? "development" : "production",
  devtool: isDevServer ? "eval" : "source-map",

  optimization: {
    minimizer: [
      new TerserWebpackPlugin({
        extractComments: false,
        terserOptions: {
          ecma: 2017,
          toplevel: true,
          output: {
            comments: false,
            preamble: "/*! gstwebrtc-api (https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/tree/main/net/webrtc/gstwebrtc-api), MPL-2.0 License, Copyright (C) 2022 Igalia S.L. <info@igalia.com>, Author: Loïc Le Page <llepage@igalia.com> */\n" +
                      "/*! Contains embedded adapter from webrtc-adapter (https://github.com/webrtcHacks/adapter), BSD 3-Clause License, Copyright (c) 2014, The WebRTC project authors. All rights reserved. Copyright (c) 2018, The adapter.js project authors. All rights reserved. */\n"
          }
        }
      })
    ]
  }
};

// Normal .js file for direct use in <script> tags
const browserConfig = {
  ...commonConfig,
  entry: { "gstwebrtc-api": "./src/index.js" },
  output: {
    filename: isDevServer ? "[name]-[contenthash].min.js" : `[name]-${packageVersion}.min.js`
  },

  devServer: {
    open: true,
    static: false,
    server: "http",
    port: 9090
  },

  plugins: [
    new webpack.ProgressPlugin(),
    new HtmlWebpackPlugin({
      template: "./index.html",
      inject: "head",
      minify: false,
      scriptLoading: "blocking"
    })
  ]
};

// ESM module for use in any modern JS project
const esmConfig = {
  ...commonConfig,
  entry: { "gstwebrtc-api": "./src/index.js" },
  output: {
    filename: `[name]-${packageVersion}.esm.js`,
    chunkFormat: "module",
    iife: false,
    library: {
      type: "module"
    },
    module: true
  },

  experiments: {
    outputModule: true
  },

  plugins: [
    new webpack.ProgressPlugin()
  ]
};

module.exports = isDevServer ? browserConfig : [browserConfig, esmConfig]; // eslint-disable-line no-undef

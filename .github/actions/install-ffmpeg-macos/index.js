// @ts-check
const core = require('@actions/core');
const exec = require('@actions/exec');
const github = require('@actions/github');

// const folders = 

exec.exec('brew', ['install', 'ffmpeg']);

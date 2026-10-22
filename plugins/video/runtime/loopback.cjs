// Loaded before Remotion Studio (`node --require .relay/loopback.cjs`) by video.preview: Studio
// binds every interface (0.0.0.0 or ::), which would put a server that can render and open files
// on the network. Here every server listens on 127.0.0.1 alone, and Studio's check for a newer
// Remotion on npm is answered at once without leaving the machine.
//
// Written by Relay from plugins/video/runtime: edits here are overwritten.
'use strict';
const net = require('node:net');
const https = require('node:https');
const { EventEmitter } = require('node:events');

const listen = net.Server.prototype.listen;
net.Server.prototype.listen = function (...args) {
  if (args[0] && typeof args[0] === 'object' && !Array.isArray(args[0])) {
    args[0] = { ...args[0], host: '127.0.0.1' };
  } else if (typeof args[0] === 'number' || typeof args[0] === 'string') {
    if (typeof args[1] === 'string') args[1] = '127.0.0.1';
    else args.splice(1, 0, '127.0.0.1');
  }
  return listen.apply(this, args);
};

const get = https.get;
https.get = function (url, ...rest) {
  const target = typeof url === 'string' ? url : url && (url.href || url.hostname || url.host);
  if (String(target).includes('registry.npmjs.org')) {
    const req = new EventEmitter();
    req.end = () => {};
    req.destroy = () => {};
    process.nextTick(() => req.emit('error', new Error('offline: Relay keeps Remotion pinned')));
    return req;
  }
  return get.call(this, url, ...rest);
};

// Loaded before Remotion Studio (`node --require .relay/loopback.cjs`) by video.preview: Studio
// binds every interface (0.0.0.0 or ::), which would put a server that can render and open files
// on the network. Here every server listens on 127.0.0.1 alone, a request that does not name it
// by that address or `localhost` (a page that rebinds its own name to 127.0.0.1) is refused, and
// Studio's check for a newer Remotion on npm is answered at once without leaving the machine.
//
// Written by Relay from plugins/video/runtime: edits here are overwritten.
'use strict';
const net = require('node:net');
const http = require('node:http');
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

// Studio trusts a request whose Origin matches its Host, which a page on another name rebound to
// 127.0.0.1 satisfies: only the loopback names, at this server's port, are let in.
const emit = http.Server.prototype.emit;
http.Server.prototype.emit = function (event, req, resOrSocket, ...rest) {
  if ((event === 'request' || event === 'upgrade') && req && req.headers) {
    const port = this.address() && this.address().port;
    const host = String(req.headers.host || '').toLowerCase();
    const allowed = [`127.0.0.1:${port}`, `localhost:${port}`, `[::1]:${port}`];
    if (!allowed.includes(host)) {
      if (event === 'request') {
        resOrSocket.statusCode = 403;
        resOrSocket.end('Forbidden: Relay serves this preview on 127.0.0.1 only');
      } else {
        resOrSocket.destroy();
      }
      return true;
    }
  }
  return emit.call(this, event, req, resOrSocket, ...rest);
};

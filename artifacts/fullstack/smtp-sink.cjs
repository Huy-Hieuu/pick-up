// Loopback-only disposable SMTP sink; discard message contents without logging.
const net = require('node:net');
let accepted = 0;
net.createServer(socket => {
  socket.setEncoding('utf8');
  socket.write('220 pickup-preview.local ESMTP\r\n');
  let buffer = '', data = false;
  socket.on('data', chunk => {
    buffer += chunk;
    while (buffer.includes('\r\n')) {
      const at = buffer.indexOf('\r\n');
      const line = buffer.slice(0, at);
      buffer = buffer.slice(at + 2);
      if (data) {
        if (line === '.') { data = false; accepted++; console.log(`Discarded preview message ${accepted}`); socket.write('250 Accepted for discard\r\n'); }
      } else if (/^(EHLO|HELO)/i.test(line)) socket.write('250-pickup-preview.local\r\n250 8BITMIME\r\n');
      else if (/^DATA/i.test(line)) { data = true; socket.write('354 End with dot\r\n'); }
      else if (/^QUIT/i.test(line)) socket.end('221 Bye\r\n');
      else socket.write('250 OK\r\n');
    }
  });
  socket.on('error', () => {});
}).listen(1025, '127.0.0.1', () => console.log('Disposable SMTP sink on 127.0.0.1:1025; no outbound delivery'));

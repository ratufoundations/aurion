import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import path from 'path';
import {defineConfig, Plugin} from 'vite';

// In-memory cooldown storage
const addressCooldowns = new Map<string, number>();
const COOLDOWN_DURATION_MS = 24 * 60 * 60 * 1000;

function generateBlake3Hex(): string {
  const chars = "0123456789abcdef";
  let hash = "0x";
  for (let i = 0; i < 64; i++) {
    hash += chars[Math.floor(Math.random() * chars.length)];
  }
  return hash;
}

function faucetApiPlugin(): Plugin {
  return {
    name: 'faucet-api-plugin',
    configureServer(server) {
      server.middlewares.use('/api/v1/claim', (req, res, next) => {
        if (req.method === 'POST') {
          let body = '';
          req.on('data', chunk => {
            body += chunk;
          });
          req.on('end', () => {
            try {
              const parsed = JSON.parse(body || '{}');
              const address = (parsed.target_address || '').trim();

              res.setHeader('Content-Type', 'application/json');

              if (!address) {
                res.statusCode = 400;
                res.end(JSON.stringify({
                  success: false,
                  error: 'Alamat address tujuan diperlukan.',
                  code: 'INVALID_ADDRESS',
                }));
                return;
              }

              // Special test trigger for node failure (Status 503)
              if (address.toLowerCase().includes('503') || address === '0x0000000000000000000000000000000000000503') {
                res.statusCode = 503;
                res.end(JSON.stringify({
                  success: false,
                  error: 'Koneksi ke Aurion Bootnode cluster gagal (Status 503). Node sedang sinkronisasi blok atau mengalami lonjakan beban transaksi.',
                  code: 'NODE_UNAVAILABLE',
                }));
                return;
              }

              // Address validation
              const isValidEvm = /^0x[a-fA-F0-9]{40}$/.test(address);
              const isValidBlake3 = /^0x[a-fA-F0-9]{64}$/.test(address);
              if (!isValidEvm && !isValidBlake3) {
                res.statusCode = 400;
                res.end(JSON.stringify({
                  success: false,
                  error: 'Format address tidak valid. Gunakan format address Hexadecimal Aurion (0x diikuti 40 atau 64 karakter hex).',
                  code: 'INVALID_ADDRESS',
                }));
                return;
              }

              const lowerAddress = address.toLowerCase();
              const now = Date.now();
              const lastClaim = addressCooldowns.get(lowerAddress);

              if (lastClaim && now - lastClaim < COOLDOWN_DURATION_MS) {
                const remainingSec = Math.ceil((COOLDOWN_DURATION_MS - (now - lastClaim)) / 1000);
                const hours = Math.floor(remainingSec / 3600);
                const mins = Math.floor((remainingSec % 3600) / 60);
                res.statusCode = 429;
                res.setHeader('Retry-After', remainingSec.toString());
                res.end(JSON.stringify({
                  success: false,
                  error: `Address ini sedang dalam periode cooldown 24 jam. Mohon tunggu sisa waktu ${hours} jam ${mins} menit sebelum meminta token kembali.`,
                  code: 'COOLDOWN_ACTIVE',
                  retry_after_seconds: remainingSec,
                  cooldown_until: lastClaim + COOLDOWN_DURATION_MS,
                }));
                return;
              }

              // Record cooldown
              addressCooldowns.set(lowerAddress, now);

              const txHash = generateBlake3Hex();
              const blockHeight = 1482900 + Math.floor(Math.random() * 500);

              res.statusCode = 200;
              res.end(JSON.stringify({
                success: true,
                message: 'Pengiriman 10 AUR berhasil dieksekusi ke jaringan Aurion Testnet.',
                tx_hash: txHash,
                amount: 10,
                symbol: 'AUR',
                block_height: blockHeight,
                timestamp: now,
                recipient: address,
                gas_fee: '0.00021 AUR',
              }));
            } catch (err: any) {
              res.statusCode = 500;
              res.end(JSON.stringify({
                success: false,
                error: 'Terjadi kesalahan internal pada faucet gateway server.',
                code: 'NODE_UNAVAILABLE',
              }));
            }
          });
        } else if (req.method === 'DELETE') {
          addressCooldowns.clear();
          res.setHeader('Content-Type', 'application/json');
          res.statusCode = 200;
          res.end(JSON.stringify({ success: true, message: 'Cooldown reset' }));
        } else {
          next();
        }
      });
    },
  };
}

export default defineConfig(() => {
  return {
    plugins: [react(), tailwindcss(), faucetApiPlugin()],
    resolve: {
      alias: {
        '@': path.resolve(__dirname, '.'),
      },
    },
    server: {
      hmr: process.env.DISABLE_HMR !== 'true',
      watch: process.env.DISABLE_HMR === 'true' ? null : {},
    },
  };
});

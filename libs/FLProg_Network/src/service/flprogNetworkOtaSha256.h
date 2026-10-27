/* Small allocation-free SHA-256/HMAC helper for the experimental STM32 OTA. */
#pragma once
#include <Arduino.h>
#include <string.h>

class FLProgOtaSha256
{
public:
    FLProgOtaSha256() { reset(); }
    void reset()
    {
        _state[0]=0x6a09e667UL; _state[1]=0xbb67ae85UL; _state[2]=0x3c6ef372UL; _state[3]=0xa54ff53aUL;
        _state[4]=0x510e527fUL; _state[5]=0x9b05688cUL; _state[6]=0x1f83d9abUL; _state[7]=0x5be0cd19UL;
        _bytes=0; _used=0;
    }
    void update(const uint8_t *data, size_t length)
    {
        _bytes += length;
        while (length)
        {
            const size_t take = (length < 64U - _used) ? length : 64U - _used;
            memcpy(_block + _used, data, take); _used += take; data += take; length -= take;
            if (_used == 64) { transform(_block); _used = 0; }
        }
    }
    void finish(uint8_t digest[32])
    {
        const uint64_t bits = _bytes * 8ULL;
        _block[_used++] = 0x80;
        if (_used > 56) { while (_used < 64) _block[_used++] = 0; transform(_block); _used = 0; }
        while (_used < 56) _block[_used++] = 0;
        for (int8_t i=7; i>=0; --i) _block[_used++] = (uint8_t)(bits >> (i*8));
        transform(_block);
        for (uint8_t i=0; i<8; ++i) { digest[i*4]=(uint8_t)(_state[i]>>24); digest[i*4+1]=(uint8_t)(_state[i]>>16); digest[i*4+2]=(uint8_t)(_state[i]>>8); digest[i*4+3]=(uint8_t)_state[i]; }
    }
private:
    static uint32_t rotr(uint32_t x, uint8_t n) { return (x >> n) | (x << (32U-n)); }
    void transform(const uint8_t block[64])
    {
        static const uint32_t k[64] = {
            0x428a2f98UL,0x71374491UL,0xb5c0fbcfUL,0xe9b5dba5UL,0x3956c25bUL,0x59f111f1UL,0x923f82a4UL,0xab1c5ed5UL,
            0xd807aa98UL,0x12835b01UL,0x243185beUL,0x550c7dc3UL,0x72be5d74UL,0x80deb1feUL,0x9bdc06a7UL,0xc19bf174UL,
            0xe49b69c1UL,0xefbe4786UL,0x0fc19dc6UL,0x240ca1ccUL,0x2de92c6fUL,0x4a7484aaUL,0x5cb0a9dcUL,0x76f988daUL,
            0x983e5152UL,0xa831c66dUL,0xb00327c8UL,0xbf597fc7UL,0xc6e00bf3UL,0xd5a79147UL,0x06ca6351UL,0x14292967UL,
            0x27b70a85UL,0x2e1b2138UL,0x4d2c6dfcUL,0x53380d13UL,0x650a7354UL,0x766a0abbUL,0x81c2c92eUL,0x92722c85UL,
            0xa2bfe8a1UL,0xa81a664bUL,0xc24b8b70UL,0xc76c51a3UL,0xd192e819UL,0xd6990624UL,0xf40e3585UL,0x106aa070UL,
            0x19a4c116UL,0x1e376c08UL,0x2748774cUL,0x34b0bcb5UL,0x391c0cb3UL,0x4ed8aa4aUL,0x5b9cca4fUL,0x682e6ff3UL,
            0x748f82eeUL,0x78a5636fUL,0x84c87814UL,0x8cc70208UL,0x90befffaUL,0xa4506cebUL,0xbef9a3f7UL,0xc67178f2UL };
        uint32_t w[64];
        for (uint8_t i=0; i<16; ++i) w[i]=((uint32_t)block[i*4]<<24)|((uint32_t)block[i*4+1]<<16)|((uint32_t)block[i*4+2]<<8)|block[i*4+3];
        for (uint8_t i=16; i<64; ++i) { uint32_t s0=rotr(w[i-15],7)^rotr(w[i-15],18)^(w[i-15]>>3); uint32_t s1=rotr(w[i-2],17)^rotr(w[i-2],19)^(w[i-2]>>10); w[i]=w[i-16]+s0+w[i-7]+s1; }
        uint32_t a=_state[0],b=_state[1],c=_state[2],d=_state[3],e=_state[4],f=_state[5],g=_state[6],h=_state[7];
        for (uint8_t i=0; i<64; ++i) { uint32_t s1=rotr(e,6)^rotr(e,11)^rotr(e,25), ch=(e&f)^(~e&g), t1=h+s1+ch+k[i]+w[i]; uint32_t s0=rotr(a,2)^rotr(a,13)^rotr(a,22), maj=(a&b)^(a&c)^(b&c), t2=s0+maj; h=g;g=f;f=e;e=d+t1;d=c;c=b;b=a;a=t1+t2; }
        _state[0]+=a;_state[1]+=b;_state[2]+=c;_state[3]+=d;_state[4]+=e;_state[5]+=f;_state[6]+=g;_state[7]+=h;
    }
    uint32_t _state[8]; uint64_t _bytes; uint8_t _block[64]; uint8_t _used;
};

class FLProgOtaHmacSha256
{
public:
    void begin(const uint8_t *key, size_t keyLength)
    {
        uint8_t keyBlock[64] = {0};
        if (keyLength > sizeof(keyBlock)) { FLProgOtaSha256 hash; hash.update(key,keyLength); hash.finish(keyBlock); }
        else if (keyLength) memcpy(keyBlock,key,keyLength);
        uint8_t inner[64];
        for (uint8_t i=0;i<64;++i) { inner[i]=keyBlock[i]^0x36; _outerPad[i]=keyBlock[i]^0x5c; }
        _inner.reset(); _inner.update(inner,sizeof(inner));
        memset(keyBlock,0,sizeof(keyBlock)); memset(inner,0,sizeof(inner));
    }
    void update(const uint8_t *data,size_t length) { _inner.update(data,length); }
    void finish(uint8_t digest[32])
    {
        uint8_t innerDigest[32]; _inner.finish(innerDigest);
        FLProgOtaSha256 outer; outer.update(_outerPad,sizeof(_outerPad)); outer.update(innerDigest,sizeof(innerDigest)); outer.finish(digest);
        memset(innerDigest,0,sizeof(innerDigest));
    }
private:
    FLProgOtaSha256 _inner; uint8_t _outerPad[64];
};


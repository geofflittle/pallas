FROM haskell:9.6.7

RUN sed -i '/debian-security/d' /etc/apt/sources.list \
    && apt-get update && apt-get install -y --no-install-recommends \
      pkg-config zlib1g-dev libtool autoconf automake \
      build-essential git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN git clone --depth 1 --branch iquerejeta/vrf_batchverify https://github.com/IntersectMBO/libsodium /tmp/sodium \
    && cd /tmp/sodium && autoreconf -fi \
    && ./configure --prefix=/usr/local && make -j4 && make install && rm -rf /tmp/sodium

RUN git clone --depth 1 --branch v0.3.2 https://github.com/bitcoin-core/secp256k1 /tmp/secp \
    && cd /tmp/secp && ./autogen.sh \
    && ./configure --prefix=/usr/local --enable-module-schnorrsig --enable-experimental \
    && make -j4 && make install && rm -rf /tmp/secp

RUN git clone --depth 1 --branch v0.3.14 https://github.com/supranational/blst /tmp/blst \
    && cd /tmp/blst && ./build.sh \
    && mkdir -p /usr/local/include /usr/local/lib/pkgconfig \
    && cp bindings/blst_aux.h bindings/blst.h bindings/blst.hpp /usr/local/include/ \
    && cp libblst.a /usr/local/lib/ \
    && printf 'prefix=/usr/local\nexec_prefix=${prefix}\nlibdir=${exec_prefix}/lib\nincludedir=${prefix}/include\n\nName: libblst\nDescription: Multilingual BLS12-381 signature library\nURL: https://github.com/supranational/blst\nVersion: 0.3.14\nCflags: -I${includedir}\nLibs: -L${libdir} -lblst\n' > /usr/local/lib/pkgconfig/libblst.pc \
    && rm -rf /tmp/blst

ENV PKG_CONFIG_PATH=/usr/local/lib/pkgconfig
ENV LD_LIBRARY_PATH=/usr/local/lib
RUN ldconfig

#define MAX_BINDLESS_STORAGE_BUFFERS 1024

[[vk::binding(0, 0)]]
RWByteAddressBuffer _bindlessStorageBuffers[MAX_BINDLESS_STORAGE_BUFFERS];

struct BindlessBuffer {
    uint id;

    // template<typename T>
    // T Load(uint i) {
    //     return _bindlessStorageBuffers[id].Load<T>(i * sizeof(T));
    // }

    float4 Load(uint i) {
        return float4(1.0, 0.0, 0.0, 0.0);//[id].Load<float4>(i * sizeof(float4));
    }
};


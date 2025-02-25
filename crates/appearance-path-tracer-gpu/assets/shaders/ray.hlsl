#ifndef RAY_H
#define RAY_H

struct Ray {
    float3 origin;
    uint _padding0;
    float3 direction;
    uint _padding1;

    static Ray _new(float3 origin, float3 direction) {
        Ray ray = (Ray)0;
        ray.origin = origin;
        ray.direction = direction;
        return ray;
    }
};

struct Payload {
    float3 accumulated;
    uint _padding0;
};

#endif
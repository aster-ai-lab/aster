// Calls the actual source functions in production_sampler.inc, extracted from
// the measured engine source. See sampler-source-provenance.json and UPSTREAM-LICENSE.
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <random>
#include <utility>
#include <vector>
#include "production_sampler.inc"

int main() {
    constexpr int N=400000,V=5;
    const float logits[V]={0.0f,1.0f,-1.0f,2.0f,0.2f};
    struct Case {float temp;int topk;float topp,minp;};
    const Case cases[]={{0,0,1,0},{0.7f,0,1,0},{1.0f,3,1,0},{0.8f,0,.8f,0},{1.2f,4,.9f,.15f},{2.0f,1,.9f,.2f}};
    int total_cases=0;
    for (int ci=0;ci<6;++ci) {
        const auto c=cases[ci];
        std::vector<std::pair<int,double>> dist;
        sample_dist(logits,V,c.temp,c.topk,c.topp,c.minp,dist);
        double probability[V]={},sum=0;
        for (auto item:dist) {if(item.first<0||item.first>=V||!std::isfinite(item.second)||item.second<0)return 1;probability[item.first]=item.second;sum+=item.second;}
        if(std::abs(sum-1)>1e-12)return 2;
        std::mt19937_64 reference_rng(1000+ci);
        long reference[V]={};
        for(int j=0;j<N;++j) {int token=sample_p(logits,V,c.temp,c.topk,c.topp,c.minp,reference_rng);if(token<0||token>=V)return 3;++reference[token];}
        for(int token=0;token<V;++token){double expected=N*probability[token],bound=6*std::sqrt(N*probability[token]*(1-probability[token]))+8;if(std::abs(reference[token]-expected)>bound)return 4;}
        for(int draft: {0,1,2,3,4,99}) {
            std::mt19937_64 rng(2000+ci*100+draft);
            long observed[V]={},accepted=0;
            for(int j=0;j<N;++j){int token=-1;bool ok=spec_accept(dist,draft,rng,token);if(ok){token=draft;++accepted;}if(token<0||token>=V)return 5;++observed[token];}
            for(int token=0;token<V;++token){double expected=N*probability[token],bound=6*std::sqrt(N*probability[token]*(1-probability[token]))+8;if(std::abs(observed[token]-expected)>bound){std::fprintf(stderr,"case=%d draft=%d token=%d observed=%ld expected=%.4f bound=%.4f\n",ci,draft,token,observed[token],expected,bound);return 6;}}
            double pd=draft<V?probability[draft]:0;
            if(std::abs(accepted-N*pd)>6*std::sqrt(N*pd*(1-pd))+8)return 7;
            ++total_cases;
            std::printf("case=%d draft=%d draws=%d accepted=%ld PASS\n",ci,draft,N,accepted);
        }
    }
    std::printf("PASS: %d distribution cases; %d draws/case; greedy, temperature, top-k, top-p, min-p and out-of-support drafts\n",total_cases,N);
    return 0;
}

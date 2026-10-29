#pragma once
#include <stdint.h>

// Pure civil-time conversion. NTP timestamps always remain UTC.
struct FLProgNtpLocalDateTime {
    int64_t localUnixTime;
    uint16_t year;
    uint8_t month,day,hours,minutes,seconds,weekDay; // Sunday=0
    int16_t offsetMinutes;
    bool daylight;
};
struct FLProgNtpDstRule {
    enum Basis { UTC=0, Standard=1, Wall=2 };
    uint8_t month,week,weekDay,hour,minute,basis; // week5=last weekday
    FLProgNtpDstRule(uint8_t m=0,uint8_t w=1,uint8_t d=0,uint8_t h=2,uint8_t n=0,uint8_t b=Wall)
        :month(m),week(w),weekDay(d),hour(h),minute(n),basis(b){}
    bool valid() const {return month>=1 && month<=12 && week>=1 && week<=5 && weekDay<=6 && hour<=23 && minute<=59 && basis<=Wall;}
};
class FLProgNtpTimeZone {
    int16_t standard_=0,summer_=0;
    FLProgNtpDstRule start_,end_;
    static bool leap(int y){return y%4==0 && (y%100!=0 || y%400==0);}
    static int monthDays(int y,int m){const uint8_t n[]={31,28,31,30,31,30,31,31,30,31,30,31};return n[m-1]+(m==2 && leap(y));}
    static int64_t daysTo(int y,int m,int d){
        int64_t days=0;
        for(int a=1970;a<y;++a)days+=leap(a)?366:365;
        for(int a=y;a<1970;++a)days-=leap(a)?366:365;
        for(int a=1;a<m;++a)days+=monthDays(y,a);
        return days+d-1;
    }
    static int weekday(int64_t days){int n=(int)((days+4)%7);return n<0?n+7:n;}
    static FLProgNtpLocalDateTime calendar(int64_t seconds){
        FLProgNtpLocalDateTime v={};v.localUnixTime=seconds;
        int64_t days=seconds/86400,part=seconds%86400;if(part<0){part+=86400;--days;}
        v.weekDay=(uint8_t)weekday(days);v.hours=(uint8_t)(part/3600);v.minutes=(uint8_t)((part%3600)/60);v.seconds=(uint8_t)(part%60);
        int y=1970;while(days<0){--y;days+=leap(y)?366:365;}
        while(days>=(leap(y)?366:365)){days-=leap(y)?366:365;++y;}
        int m=1;while(days>=monthDays(y,m)){days-=monthDays(y,m);++m;}
        v.year=(uint16_t)y;v.month=(uint8_t)m;v.day=(uint8_t)(days+1);return v;
    }
    int64_t transition(int year,const FLProgNtpDstRule &r,bool ending=false) const {
        int day;
        if(r.week==5){int last=monthDays(year,r.month);day=last-(weekday(daysTo(year,r.month,last))-r.weekDay+7)%7;}
        else day=1+(r.weekDay-weekday(daysTo(year,r.month,1))+7)%7+7*(r.week-1);
        int before=r.basis==FLProgNtpDstRule::UTC?0:standard_;
        if(r.basis==FLProgNtpDstRule::Wall && ending)before+=summer_;
        return daysTo(year,r.month,day)*86400+(int64_t)r.hour*3600+(int64_t)r.minute*60-(int64_t)before*60;
    }
    static bool offsetValid(int n){return n>=-720 && n<=840;}
public:
    bool setFixed(int minutes){if(!offsetValid(minutes))return false;standard_=(int16_t)minutes;summer_=0;return true;}
    bool setRules(int standardMinutes,int summerMinutes,const FLProgNtpDstRule &start,const FLProgNtpDstRule &end){
        if(!offsetValid(standardMinutes)||summerMinutes==0||summerMinutes< -180||summerMinutes>180||!offsetValid(standardMinutes+summerMinutes)||!start.valid()||!end.valid())return false;
        if(start.month==end.month && start.week==end.week && start.weekDay==end.weekDay && start.hour==end.hour && start.minute==end.minute && start.basis==end.basis)return false;
        standard_=(int16_t)standardMinutes;summer_=(int16_t)summerMinutes;start_=start;end_=end;return true;
    }
    bool setEuropean(int minutes){return setRules(minutes,60,{3,5,0,1,0,FLProgNtpDstRule::UTC},{10,5,0,1,0,FLProgNtpDstRule::UTC});}
    bool setUS(int minutes){return setRules(minutes,60,{3,2,0,2,0,FLProgNtpDstRule::Wall},{11,1,0,2,0,FLProgNtpDstRule::Wall});}
    bool isDaylight(uint32_t utc) const {
        if(!summer_)return false;
        int year=calendar(utc).year;int64_t latest=INT64_MIN;bool active=false;
        // Adjacent years handle southern seasons and transitions crossing UTC New Year.
        for(int y=year-1;y<=year+1;++y){
            int64_t a=transition(y,start_),b=transition(y,end_,true);
            if(a<=utc && a>latest){latest=a;active=true;}
            if(b<=utc && b>=latest){latest=b;active=false;}
        }
        return active;
    }
    int16_t offsetMinutes(uint32_t utc) const {return (int16_t)(standard_+(isDaylight(utc)?summer_:0));}
    FLProgNtpLocalDateTime local(uint32_t utc) const {
        bool dst=isDaylight(utc);int offset=standard_+(dst?summer_:0);
        FLProgNtpLocalDateTime value=calendar((int64_t)utc+(int64_t)offset*60);value.offsetMinutes=(int16_t)offset;value.daylight=dst;return value;
    }
};

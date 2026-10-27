#pragma once

#include "RT_HW_Timer.hpp"
#include "RT_HW_CONSOLE.h"  // Подключаем внешний интерфейс для вывода

class RT_HW_shedTask {
public:
    struct Task {
        int id;
        int priority;
        RT_HW_Timer::Callback callback;
        bool once;
        bool paused;
        int remainingRuns; // -1 = бесконечно
        uint32_t interval;
    };

    RT_HW_Timer timer;

    // Добавить периодическую задачу
    int add(uint32_t intervalMs, RT_HW_Timer::Callback cb, int priority = 0, int count = -1) {
        return internalAdd(intervalMs, cb, priority, false, count);
    }

    // Добавить одноразовую задачу
    int addOnce(uint32_t delayMs, RT_HW_Timer::Callback cb, int priority = 0) {
        return internalAdd(delayMs, cb, priority, true, 1);
    }

    // Удалить задачу
    bool remove(int id) {
        timer.cancel(id);
        for (int i = 0; i < taskCount; ++i) {
            if (tasks[i].id == id) {
                for (int j = i; j < taskCount - 1; ++j)
                    tasks[j] = tasks[j + 1];
                --taskCount;
                return true;
            }
        }
        return false;
    }

    bool pause(int id) {
        Task* t = find(id);
        if (t) {
            t->paused = true;
            return true;
        }
        return false;
    }

    bool resume(int id) {
        Task* t = find(id);
        if (t) {
            t->paused = false;
            return true;
        }
        return false;
    }

    void update() {
        timer.update();
    }

    // Метод вывода информации о всех задачах
    void debugAll() {
        for (int i = 0; i < taskCount; ++i) {
            const Task& t = tasks[i];
            RT_HW_CONSOLE.printTaskInfo(
                t.id,
                t.interval,
                t.paused,
                t.priority,
                t.once,
                t.remainingRuns
            );
        }
    }

private:
    Task* tasks = nullptr;
    int taskCount = 0;
    int taskCapacity = 0;

    void expand() {
        int newCap = taskCapacity ? taskCapacity * 2 : 4;
        Task* newTasks = (Task*)realloc(tasks, sizeof(Task) * newCap);
        if (newTasks) {
            tasks = newTasks;
            taskCapacity = newCap;
        }
    }

    Task* find(int id) {
        for (int i = 0; i < taskCount; ++i)
            if (tasks[i].id == id) return &tasks[i];
        return nullptr;
    }

    int internalAdd(uint32_t intervalMs, RT_HW_Timer::Callback cb, int priority, bool once, int count) {
        int id = timer.every(intervalMs, [this, cb, once, count, id = -1]() mutable {
            Task* task = find(id);
            if (task && !task->paused) {
                task->callback();
                if (task->once || (task->remainingRuns > 0 && --task->remainingRuns == 0)) {
                    remove(id);
                }
            }
        });

        if (id >= 0) {
            if (taskCount >= taskCapacity) expand();
            tasks[taskCount++] = {id, priority, cb, once, false, count, intervalMs};
        }

        return id;
    }
};

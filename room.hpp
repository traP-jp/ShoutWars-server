#pragma once

#include <nlohmann/json.hpp>
#include <boost/uuid/time_generator_v7.hpp>
#include <boost/uuid/uuid.hpp>
#include <chrono>
#include <thread>
#include <atomic>
#include <shared_mutex>
#include <mutex>
#include <map>
#include <string>
#include <list>
#include <functional>
#include <memory>

class room_t {
public:
  class user_t {
    friend class room_t;

  public:
    const boost::uuids::uuid id;

    explicit user_t(std::string name);

    user_t(const user_t &other);

    friend void to_json(nlohmann::json &j, const user_t &user);

    std::string get_name() const;

    void set_name(std::string new_name);

  protected:
    std::string name;
    mutable std::shared_mutex name_mutex;

    std::atomic<std::chrono::steady_clock::time_point> last_time;
  };

  using logger = std::function<void(const std::string &)>;

  const logger log_error, log_info;

  const boost::uuids::uuid id;
  const std::string version;
  const size_t size;

  explicit room_t(
    std::string version, const user_t &owner, size_t size, logger log_error = [](const std::string &) {},
    logger log_info = [](const std::string &) {}
  );

  void join(std::string version, const user_t &user);

  user_t get_user(boost::uuids::uuid id) const;

  bool has_user(boost::uuids::uuid id) const;

  bool kick(boost::uuids::uuid id);

  size_t kick_expired(std::chrono::milliseconds timeout);

  std::list<user_t> get_users() const;

  user_t get_owner() const;

  bool is_in_lobby() const;

  void start_game();

  const nlohmann::json &get_info() const;

  void update_info(const nlohmann::json &new_info);

protected:
  static boost::uuids::time_generator_v7 gen_id;

  std::list<user_t> users;
  mutable std::shared_mutex users_mutex;

  std::atomic<bool> in_lobby = true;

  nlohmann::json info;
  mutable std::mutex info_mutex;
};

class room_list_t {
public:
  using logger = std::function<void(const std::string &)>;

  const logger log_error, log_info;
  std::atomic<size_t> limit;

  explicit room_list_t(
    size_t limit, logger log_error = [](const std::string &) {}, logger log_info = [](const std::string &) {}
  );

  std::shared_ptr<room_t> create(const std::string &version, const room_t::user_t &owner, size_t size);

  std::shared_ptr<room_t> get(boost::uuids::uuid id) const;

  bool exists(boost::uuids::uuid id) const;

  bool remove(boost::uuids::uuid id);

  size_t count() const;

  std::list<std::shared_ptr<room_t>> get_all() const;

  bool start_cleaner(std::chrono::milliseconds interval, std::chrono::milliseconds timeout);

  void stop_cleaner();

protected:
  std::map<boost::uuids::uuid, std::shared_ptr<room_t>> rooms;
  mutable std::shared_mutex rooms_mutex;

  std::thread cleaner;
  std::atomic<bool> is_cleaner_running = false;
  mutable std::mutex cleaner_mutex;
};

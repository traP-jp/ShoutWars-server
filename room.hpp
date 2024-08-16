#pragma once

#include <nlohmann/json.hpp>
#include <boost/uuid/time_generator_v7.hpp>
#include <boost/uuid/uuid.hpp>
#include <chrono>
#include <thread>
#include <mutex>
#include <map>
#include <string>
#include <vector>
#include <functional>
#include <memory>

class room_t {
public:
  class user_t {
  public:
    const boost::uuids::uuid id;

    explicit user_t(std::string name);

    user_t(const user_t &);

    std::string get_name() const;

    void set_name(std::string new_name);

  protected:
    std::string name;
    mutable std::mutex name_mutex;

    std::chrono::steady_clock::time_point last_time;
  };

  const boost::uuids::uuid id;
  const std::string version;
  const size_t size;

  explicit room_t(std::string version, const user_t &owner, size_t size);

protected:
  static boost::uuids::time_generator_v7 gen_id;

  std::vector<user_t> users;
  mutable std::mutex users_mutex;

  bool in_lobby = true;

  nlohmann::json info;
  mutable std::mutex info_mutex;
};

class room_list_t {
public:
  using logger = std::function<void(const std::string &)>;

  logger log_info, log_error;
  size_t limit;

  explicit room_list_t(logger log_info, logger log_error, size_t limit);

  std::shared_ptr<room_t> create(const std::string &version, const room_t::user_t &owner, size_t size);

  std::shared_ptr<room_t> get(boost::uuids::uuid id) const;

  bool exists(boost::uuids::uuid id) const;

  void remove(boost::uuids::uuid id);

  size_t count() const;

  std::vector<std::shared_ptr<room_t>> get_all() const;

  bool start_cleaner(std::chrono::milliseconds interval, std::chrono::milliseconds timeout);

  void stop_cleaner();

protected:
  std::map<boost::uuids::uuid, std::shared_ptr<room_t>> rooms;
  mutable std::mutex rooms_mutex;

  std::thread cleaner;
  bool is_cleaner_running = false;
};
